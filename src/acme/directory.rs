use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct Directory {
    pub new_nonce: String,
    pub new_account: String,
    pub new_order: String,
    pub revoke_cert: String,
    pub key_change: String,
}

impl Directory {
    pub async fn fetch(client: &Client, url: &str) -> Result<Self> {
        let dir = client
            .get(url)
            .send()
            .await
            .context("Failed to fetch ACME directory")?
            .json::<Directory>()
            .await
            .context("Failed to parse ACME directory")?;
        tracing::debug!("Directory fetched: {:?}", dir);
        Ok(dir)
    }
}

#[cfg(test)]
mod tests {
    #[allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn test_directory_deserialization() {
        let json = r#"{
            "newNonce":   "https://acme.example.com/new-nonce",
            "newAccount": "https://acme.example.com/new-account",
            "newOrder":   "https://acme.example.com/new-order",
            "revokeCert": "https://acme.example.com/revoke-cert",
            "keyChange":  "https://acme.example.com/key-change"
        }"#;
        let dir: Directory = serde_json::from_str(json).unwrap();
        assert_eq!(dir.new_nonce, "https://acme.example.com/new-nonce");
        assert_eq!(dir.new_account, "https://acme.example.com/new-account");
        assert_eq!(dir.new_order, "https://acme.example.com/new-order");
        assert_eq!(dir.revoke_cert, "https://acme.example.com/revoke-cert");
        assert_eq!(dir.key_change, "https://acme.example.com/key-change");
    }

    #[test]
    fn test_directory_deserialization_rejects_missing_field() {
        let json = r#"{"newNonce": "https://acme.example.com/new-nonce"}"#;
        let result: Result<Directory, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
