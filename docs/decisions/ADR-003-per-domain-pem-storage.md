# ADR-003: Store Certificates Per-Domain as PEM Files

## Status
Accepted

## Date
2026-06-29

## Context

The ACME client must persist the private key and certificate chain it receives from
Let's Encrypt (or Pebble) so they can be used by a web server or inspected later.
Several storage formats were considered:

- **PKCS#12 / PFX** — single file containing key + cert; requires `openssl` tooling to
  inspect; not natively readable by most web server configs.
- **JKS (Java KeyStore)** — Java-ecosystem format; no Rust crates exist for writing it;
  irrelevant outside the JVM.
- **Single flat PEM file** — simpler but mixes key and cert; non-standard layout that
  differs from how Certbot, acme.sh, and other ACME clients store files.
- **Per-domain PEM directory** mirroring Certbot's layout:
  `certs/<domain>/privkey.pem`, `cert.pem`, `fullchain.pem`.

## Decision

Store certificates under `certs/<domain>/` with three files:
- `privkey.pem` — the domain private key (PKCS#8 PEM)
- `cert.pem` — the leaf certificate only (first PEM block from the chain)
- `fullchain.pem` — the full certificate chain as returned by the ACME server

Wildcard domains (`*.example.com`) have `*` replaced with `wildcard` in the directory name
to avoid filesystem path issues on Windows and macOS.

## Consequences

### Positive
- Drop-in compatible with nginx, Apache, and HAProxy configs that expect Certbot-style paths.
- `cert.pem` and `fullchain.pem` are separate, avoiding the common confusion between leaf
  and chain in TLS server configuration.
- Easy to inspect with `openssl x509 -in certs/example.com/cert.pem -text`.
- `certs/` is gitignored, preventing accidental private key exposure.

### Negative / Trade-offs
- File permissions must be managed explicitly; the `certs/` directory is world-readable by
  default unless the caller restricts it (e.g., `chmod 600 privkey.pem`).
- No atomic write — a crash mid-write could leave the domain directory in a partial state.
  Renewals overwrite files in place rather than using a staging-and-swap pattern.

### Neutral
- The `show` CLI subcommand reads `cert.pem` via `x509-parser` to display validity dates,
  issuer, and SANs without requiring `openssl` on the host.
