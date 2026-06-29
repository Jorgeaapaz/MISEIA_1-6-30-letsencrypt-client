# ADR-001: Use ECDSA P-256 (ES256) for ACME Account Keys

## Status
Accepted

## Date
2026-06-29

## Context

RFC 8555 (ACME) requires that account key operations use a supported JSON Web Algorithm.
Both RSA (RS256) and ECDSA P-256 (ES256) are valid. The account key is used for every
JWS-signed request to the ACME server, so key size and signature performance matter.

The `ring` crate (our cryptography dependency) provides ECDSA P-256 as a first-class,
audited primitive. RSA support in `ring` requires additional key-generation plumbing and
produces significantly larger keys (2048-bit minimum = 256 bytes private key vs. 32 bytes
for P-256). Let's Encrypt and Pebble both support ES256.

## Decision

Use ECDSA P-256 (algorithm identifier ES256) for all ACME account keys.
Key generation and JWS signing are implemented via `ring::signature::EcdsaKeyPair`
with `ECDSA_P256_SHA256_FIXED_SIGNING`.

## Consequences

### Positive
- Account private key is 32 bytes (vs. 256 bytes for RSA-2048), smaller storage footprint.
- Signature generation is faster than RSA at equivalent security levels.
- `ring` provides a safe, audited API; no unsafe RSA padding decisions to make.
- JWK thumbprint computation is straightforward over the canonical `{crv, kty, x, y}` object.

### Negative / Trade-offs
- No RSA account key support — clients that require RSA cannot reuse this key material.
- ECDSA signatures are non-deterministic (each signing produces a different `r, s` pair),
  making test-time reproducibility harder; tests verify structure, not byte equality.

### Neutral
- CSR key (separate from account key) also uses P-256 via `rcgen`, keeping the
  cryptographic surface consistent throughout the codebase.
