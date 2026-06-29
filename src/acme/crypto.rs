use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ring::{
    rand::SystemRandom,
    signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// ECDSA P-256 account key pair.
pub struct AccountKey {
    pub key_pair: EcdsaKeyPair,
    pub pkcs8_der: Vec<u8>,
}

impl AccountKey {
    /// Generate a new ECDSA P-256 key pair.
    pub fn generate() -> Result<Self> {
        let rng = SystemRandom::new();
        let pkcs8_doc = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
            .map_err(|e| anyhow::anyhow!("Failed to generate ECDSA key pair: {:?}", e))?;
        let pkcs8_der = pkcs8_doc.as_ref().to_vec();
        let key_pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &pkcs8_der, &rng)
            .map_err(|e| anyhow::anyhow!("Failed to load generated key pair: {:?}", e))?;
        Ok(Self {
            key_pair,
            pkcs8_der,
        })
    }

    /// Load from PKCS#8 DER bytes.
    pub fn from_pkcs8(der: &[u8]) -> Result<Self> {
        let rng = SystemRandom::new();
        let key_pair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, der, &rng)
            .map_err(|e| anyhow::anyhow!("Failed to load ECDSA key pair from PKCS8: {:?}", e))?;
        Ok(Self {
            key_pair,
            pkcs8_der: der.to_vec(),
        })
    }

    /// Return the JWK public key as a JSON Value (P-256, ES256).
    pub fn jwk(&self) -> Value {
        let public_key = self.key_pair.public_key().as_ref();
        // Uncompressed point: 0x04 || x (32 bytes) || y (32 bytes)
        assert_eq!(public_key[0], 0x04, "Expected uncompressed EC point");
        let x = URL_SAFE_NO_PAD.encode(&public_key[1..33]);
        let y = URL_SAFE_NO_PAD.encode(&public_key[33..65]);
        json!({
            "crv": "P-256",
            "kty": "EC",
            "x": x,
            "y": y
        })
    }

    /// Compute the JWK thumbprint (SHA-256 over canonical JWK).
    pub fn jwk_thumbprint(&self) -> Result<String> {
        let jwk = self.jwk();
        // Canonical form: sorted keys, no whitespace
        let canonical = serde_json::to_string(&jwk).context("Failed to serialize JWK")?;
        let mut hasher = Sha256::new();
        hasher.update(canonical.as_bytes());
        let digest = hasher.finalize();
        Ok(URL_SAFE_NO_PAD.encode(digest))
    }

    /// Build a JWS (Flattened JSON Serialization) for an ACME POST request.
    /// - `payload`: the JSON body, or `None` for POST-as-GET (empty string payload)
    /// - `url`: the target URL
    /// - `nonce`: replay nonce from server
    /// - `kid`: account URL if known; otherwise JWK is embedded
    pub fn sign_jws(
        &self,
        payload: Option<&Value>,
        url: &str,
        nonce: &str,
        kid: Option<&str>,
    ) -> Result<Value> {
        let rng = SystemRandom::new();

        // Header
        let header = if let Some(kid_url) = kid {
            json!({
                "alg": "ES256",
                "nonce": nonce,
                "url": url,
                "kid": kid_url
            })
        } else {
            json!({
                "alg": "ES256",
                "nonce": nonce,
                "url": url,
                "jwk": self.jwk()
            })
        };

        let header_b64 =
            URL_SAFE_NO_PAD.encode(serde_json::to_string(&header).context("header serialize")?);

        let payload_b64 = match payload {
            Some(p) => {
                URL_SAFE_NO_PAD.encode(serde_json::to_string(p).context("payload serialize")?)
            }
            None => String::new(), // POST-as-GET
        };

        let signing_input = format!("{}.{}", header_b64, payload_b64);
        let sig = self
            .key_pair
            .sign(&rng, signing_input.as_bytes())
            .map_err(|_| anyhow::anyhow!("Failed to sign JWS"))?;

        let sig_b64 = URL_SAFE_NO_PAD.encode(sig.as_ref());

        Ok(json!({
            "protected": header_b64,
            "payload": payload_b64,
            "signature": sig_b64
        }))
    }
}

#[cfg(test)]
mod tests {
    #[allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_generate_account_key_roundtrip() {
        let key = AccountKey::generate().unwrap();
        assert!(!key.pkcs8_der.is_empty());
        let key2 = AccountKey::from_pkcs8(&key.pkcs8_der).unwrap();
        assert_eq!(key2.pkcs8_der, key.pkcs8_der);
    }

    #[test]
    fn test_jwk_has_required_fields() {
        let key = AccountKey::generate().unwrap();
        let jwk = key.jwk();
        assert_eq!(jwk["kty"], "EC");
        assert_eq!(jwk["crv"], "P-256");
        assert!(jwk["x"].is_string());
        assert!(jwk["y"].is_string());
    }

    #[test]
    fn test_jwk_thumbprint_deterministic() {
        let key = AccountKey::generate().unwrap();
        let t1 = key.jwk_thumbprint().unwrap();
        let t2 = key.jwk_thumbprint().unwrap();
        assert_eq!(t1, t2);
        assert!(!t1.is_empty());
    }

    #[test]
    fn test_jws_structure_with_payload() {
        let key = AccountKey::generate().unwrap();
        let payload = serde_json::json!({"resource": "new-account"});
        let jws = key
            .sign_jws(
                Some(&payload),
                "https://acme.example.com/new-account",
                "nonce123",
                None,
            )
            .unwrap();
        assert!(jws["protected"].is_string());
        assert!(jws["payload"].is_string());
        assert!(jws["signature"].is_string());
        // Without kid, JWK must be embedded in the header
        let header_b64 = jws["protected"].as_str().unwrap();
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let header_json = URL_SAFE_NO_PAD.decode(header_b64).unwrap();
        let header: serde_json::Value = serde_json::from_slice(&header_json).unwrap();
        assert!(header["jwk"].is_object());
        assert!(header.get("kid").is_none());
    }

    #[test]
    fn test_jws_post_as_get_with_kid() {
        let key = AccountKey::generate().unwrap();
        let jws = key
            .sign_jws(
                None,
                "https://acme.example.com/order/1",
                "nonce456",
                Some("https://acme.example.com/account/1"),
            )
            .unwrap();
        // POST-as-GET: payload must be the empty string
        assert_eq!(jws["payload"].as_str().unwrap(), "");
        let header_b64 = jws["protected"].as_str().unwrap();
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        let header_json = URL_SAFE_NO_PAD.decode(header_b64).unwrap();
        let header: serde_json::Value = serde_json::from_slice(&header_json).unwrap();
        assert_eq!(
            header["kid"].as_str().unwrap(),
            "https://acme.example.com/account/1"
        );
        assert!(header.get("jwk").is_none());
    }
}
