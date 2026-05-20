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
