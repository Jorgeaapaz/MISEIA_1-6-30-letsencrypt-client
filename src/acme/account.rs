use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

use super::client::AcmeClient;
use super::crypto::AccountKey;

#[derive(Debug, Serialize, Deserialize)]
pub struct AccountInfo {
    pub account_url: String,
    /// PKCS#8 DER bytes encoded as hex
    pub pkcs8_hex: String,
}

impl AccountInfo {
    fn path(accounts_dir: &Path) -> PathBuf {
        accounts_dir.join("account.json")
    }

    pub fn save(&self, accounts_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(accounts_dir).context("create accounts dir")?;
        let json = serde_json::to_string_pretty(self).context("serialize AccountInfo")?;
        std::fs::write(Self::path(accounts_dir), json).context("write account.json")?;
        tracing::info!("Account saved to {:?}", Self::path(accounts_dir));
        Ok(())
    }

    pub fn load(accounts_dir: &Path) -> Result<Option<Self>> {
        let p = Self::path(accounts_dir);
        if !p.exists() {
            return Ok(None);
        }
        let json = std::fs::read_to_string(&p).context("read account.json")?;
        let info = serde_json::from_str(&json).context("parse account.json")?;
        Ok(Some(info))
    }
}

/// Create or load an ACME account. Returns the mutated AcmeClient with account_url set.
pub async fn ensure_account(
    client: &mut AcmeClient,
    email: &str,
    accounts_dir: &Path,
) -> Result<()> {
    if let Some(info) = AccountInfo::load(accounts_dir)? {
        tracing::info!("Loaded existing account: {}", info.account_url);
        let der = hex::decode(&info.pkcs8_hex).context("decode pkcs8 hex")?;
        // Replace key in client
        client.account_key = AccountKey::from_pkcs8(&der)?;
        client.account_url = Some(info.account_url);
        return Ok(());
    }

    tracing::info!("Creating new ACME account for {}", email);
    let payload = json!({
        "termsOfServiceAgreed": true,
        "contact": [format!("mailto:{}", email)]
    });

    let resp = client.post(&client.directory.new_account.clone(), Some(&payload)).await?;

    if !resp.status().is_success() && resp.status().as_u16() != 201 {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Failed to create account: {} — {}", status, body);
    }

    let account_url = resp
        .headers()
        .get("Location")
        .context("Missing Location header in newAccount response")?
        .to_str()
        .context("Invalid Location header")?
        .to_owned();

    tracing::info!("Account created: {}", account_url);

    let pkcs8_hex = hex::encode(&client.account_key.pkcs8_der);
    let info = AccountInfo {
        account_url: account_url.clone(),
        pkcs8_hex,
    };
    info.save(accounts_dir)?;

    client.account_url = Some(account_url);
    Ok(())
}
