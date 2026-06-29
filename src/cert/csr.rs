use anyhow::{Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rcgen::{CertificateParams, DnType, KeyPair};

pub struct GeneratedCsr {
    /// DER-encoded CSR, base64url (no padding) — for ACME finalize
    pub csr_b64: String,
    /// PEM-encoded private key for the certificate (different from account key)
    pub private_key_pem: String,
}

/// Generate a new key pair and CSR for the given domains (first domain is CN, all go in SAN).
pub fn generate_csr(domains: &[String]) -> Result<GeneratedCsr> {
    let mut params =
        CertificateParams::new(domains.to_vec()).context("Failed to create CertificateParams")?;

    params
        .distinguished_name
        .push(DnType::CommonName, domains[0].clone());

    let key_pair = KeyPair::generate().context("Failed to generate certificate key pair")?;
    let private_key_pem = key_pair.serialize_pem();

    let csr = params
        .serialize_request(&key_pair)
        .context("Failed to serialize CSR")?;

    let csr_b64 = URL_SAFE_NO_PAD.encode(csr.der().as_ref());

    Ok(GeneratedCsr {
        csr_b64,
        private_key_pem,
    })
}

#[cfg(test)]
mod tests {
    #[allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_generate_csr_single_domain() {
        let domains = vec!["test.example.com".to_string()];
        let result = generate_csr(&domains).unwrap();
        assert!(!result.csr_b64.is_empty(), "CSR base64 must not be empty");
        assert!(
            result.private_key_pem.contains("PRIVATE KEY"),
            "private key PEM must contain PRIVATE KEY header"
        );
    }

    #[test]
    fn test_generate_csr_multi_domain() {
        let domains = vec![
            "test1.example.com".to_string(),
            "www.test1.example.com".to_string(),
        ];
        let result = generate_csr(&domains).unwrap();
        assert!(!result.csr_b64.is_empty());
        assert!(!result.private_key_pem.is_empty());
    }

    #[test]
    fn test_generate_csr_produces_unique_keys() {
        let domains = vec!["test.example.com".to_string()];
        let r1 = generate_csr(&domains).unwrap();
        let r2 = generate_csr(&domains).unwrap();
        // Two calls must produce different keys (fresh key pair each time)
        assert_ne!(r1.private_key_pem, r2.private_key_pem);
    }
}
