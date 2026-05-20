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
