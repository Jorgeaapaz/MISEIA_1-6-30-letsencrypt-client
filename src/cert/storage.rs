use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use x509_parser::prelude::*;

pub struct CertPaths {
    pub privkey: PathBuf,
    pub cert: PathBuf,
    pub fullchain: PathBuf,
}

impl CertPaths {
    pub fn for_domain(certs_dir: &Path, domain: &str) -> Self {
        let safe = domain.replace('*', "wildcard").replace('/', "_");
        let dir = certs_dir.join(&safe);
        Self {
            privkey: dir.join("privkey.pem"),
            cert: dir.join("cert.pem"),
            fullchain: dir.join("fullchain.pem"),
        }
    }
}

/// Save the certificate chain PEM and private key PEM to disk.
pub fn save_certificate(
    certs_dir: &Path,
    domain: &str,
    private_key_pem: &str,
    cert_chain_pem: &str,
) -> Result<CertPaths> {
    let paths = CertPaths::for_domain(certs_dir, domain);
    let dir = paths
        .privkey
        .parent()
        .context("privkey path has no parent directory")?;
    std::fs::create_dir_all(dir).context("create cert dir")?;

    let certs = split_pem_certs(cert_chain_pem);
    let leaf = certs.first().context("No certificate in chain")?;

    std::fs::write(&paths.privkey, private_key_pem).context("write privkey.pem")?;
    std::fs::write(&paths.cert, leaf).context("write cert.pem")?;
    std::fs::write(&paths.fullchain, cert_chain_pem).context("write fullchain.pem")?;

    tracing::info!("Certificates saved to {:?}", dir);
    Ok(paths)
}

fn split_pem_certs(pem: &str) -> Vec<String> {
    let mut certs = Vec::new();
    let mut current = String::new();
    for line in pem.lines() {
        current.push_str(line);
        current.push('\n');
        if line == "-----END CERTIFICATE-----" {
            certs.push(current.clone());
            current.clear();
        }
    }
    certs
}

/// Display information about a stored certificate.
pub fn show_certificate(certs_dir: &Path, domain: &str) -> Result<()> {
    let paths = CertPaths::for_domain(certs_dir, domain);
    if !paths.cert.exists() {
        anyhow::bail!("No certificate found for domain '{}'", domain);
    }

    let pem_data = std::fs::read_to_string(&paths.cert).context("read cert.pem")?;

    // Use x509-parser's built-in PEM parsing
    let (_, pem) = x509_parser::pem::parse_x509_pem(pem_data.as_bytes())
        .map_err(|e| anyhow::anyhow!("Failed to parse PEM: {:?}", e))?;
    let (_, cert) = X509Certificate::from_der(pem.contents.as_slice())
        .map_err(|e| anyhow::anyhow!("Failed to parse X509: {:?}", e))?;

    println!("Domain       : {}", domain);
    println!("Subject      : {}", cert.subject());
    println!("Issuer       : {}", cert.issuer());
    println!("Not Before   : {}", cert.validity().not_before);
    println!("Not After    : {}", cert.validity().not_after);
    println!("Serial       : {}", cert.serial);

    let san: Vec<String> = cert
        .subject_alternative_name()
        .ok()
        .flatten()
        .map(|ext| {
            ext.value
                .general_names
                .iter()
                .filter_map(|n| {
                    if let GeneralName::DNSName(s) = n {
                        Some(s.to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    if !san.is_empty() {
        println!("SANs         : {}", san.join(", "));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[allow(clippy::unwrap_used)]
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const FAKE_KEY: &str = "-----BEGIN PRIVATE KEY-----\nZmFrZWtleQ==\n-----END PRIVATE KEY-----\n";
    const FAKE_CERT: &str =
        "-----BEGIN CERTIFICATE-----\nZmFrZWNlcnQ=\n-----END CERTIFICATE-----\n";

    #[test]
    fn test_save_certificate_creates_all_three_files() {
        let tmp = TempDir::new().unwrap();
        let paths = save_certificate(tmp.path(), "test.example.com", FAKE_KEY, FAKE_CERT).unwrap();
        assert!(paths.privkey.exists(), "privkey.pem must exist");
        assert!(paths.cert.exists(), "cert.pem must exist");
        assert!(paths.fullchain.exists(), "fullchain.pem must exist");
    }

    #[test]
    fn test_save_certificate_content_is_correct() {
        let tmp = TempDir::new().unwrap();
        let paths = save_certificate(tmp.path(), "test.example.com", FAKE_KEY, FAKE_CERT).unwrap();
        assert_eq!(fs::read_to_string(&paths.privkey).unwrap(), FAKE_KEY);
        assert_eq!(fs::read_to_string(&paths.fullchain).unwrap(), FAKE_CERT);
    }

    #[test]
    fn test_save_certificate_splits_chain_correctly() {
        let chain = format!("{}{}", FAKE_CERT, FAKE_CERT);
        let tmp = TempDir::new().unwrap();
        let paths = save_certificate(tmp.path(), "chain.example.com", FAKE_KEY, &chain).unwrap();
        // cert.pem must be only the leaf (first block)
        assert_eq!(fs::read_to_string(&paths.cert).unwrap(), FAKE_CERT);
        // fullchain.pem must be the entire chain
        assert_eq!(fs::read_to_string(&paths.fullchain).unwrap(), chain);
    }

    #[test]
    fn test_cert_paths_sanitises_wildcard() {
        let base = std::path::PathBuf::from("/tmp/certs");
        let paths = CertPaths::for_domain(&base, "*.example.com");
        let privkey_str = paths.privkey.to_string_lossy();
        assert!(
            privkey_str.contains("wildcard"),
            "wildcard must replace '*'"
        );
        assert!(!privkey_str.contains('*'), "'*' must not remain in path");
    }
}
