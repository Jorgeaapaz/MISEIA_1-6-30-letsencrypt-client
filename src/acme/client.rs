use anyhow::{Context, Result};
use reqwest::{header, Response};
use serde_json::Value;

use super::{crypto::AccountKey, directory::Directory};

pub struct AcmeClient {
    pub http: reqwest::Client,
    pub directory: Directory,
    pub account_key: AccountKey,
    pub account_url: Option<String>,
}

impl AcmeClient {
    pub async fn new(acme_url: &str, account_key: AccountKey, insecure_tls: bool) -> Result<Self> {
        let http = build_http_client(insecure_tls)?;
        let directory = Directory::fetch(&http, acme_url).await?;
        Ok(Self {
            http,
            directory,
            account_key,
            account_url: None,
        })
    }

    /// Fetch a fresh nonce from the server.
    pub async fn fresh_nonce(&self) -> Result<String> {
        let resp = self
            .http
            .head(&self.directory.new_nonce)
            .send()
            .await
            .context("Failed to fetch nonce")?;
        extract_nonce(&resp)
    }

    /// Perform an ACME POST request (JWS-signed).
    /// Uses kid when account_url is set, otherwise embeds JWK.
    pub async fn post(&self, url: &str, payload: Option<&Value>) -> Result<Response> {
        let nonce = self.fresh_nonce().await?;
        let kid = self.account_url.as_deref();
        let jws = self.account_key.sign_jws(payload, url, &nonce, kid)?;

        let resp = self
            .http
            .post(url)
            .header(header::CONTENT_TYPE, "application/jose+json")
            .json(&jws)
            .send()
            .await
            .context(format!("POST to {} failed", url))?;

        tracing::debug!("POST {} -> {}", url, resp.status());
        Ok(resp)
    }

    /// POST-as-GET (empty payload, kid required).
    pub async fn post_as_get(&self, url: &str) -> Result<Response> {
        let nonce = self.fresh_nonce().await?;
        let kid = self.account_url.as_deref();
        let jws = self.account_key.sign_jws(None, url, &nonce, kid)?;

        let resp = self
            .http
            .post(url)
            .header(header::CONTENT_TYPE, "application/jose+json")
            .json(&jws)
            .send()
            .await
            .context(format!("POST-as-GET to {} failed", url))?;

        tracing::debug!("POST-as-GET {} -> {}", url, resp.status());
        Ok(resp)
    }
}

pub fn extract_nonce(resp: &Response) -> Result<String> {
    resp.headers()
        .get("Replay-Nonce")
        .context("Missing Replay-Nonce header")?
        .to_str()
        .context("Invalid Replay-Nonce header")
        .map(str::to_owned)
}

fn build_http_client(insecure_tls: bool) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder().user_agent("acme-client-rust/0.1");

    if insecure_tls {
        builder = builder.danger_accept_invalid_certs(true);
    }

    builder.build().context("Failed to build HTTP client")
}
