use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;

use super::challenge::{key_authorization, ChallengeServer};
use super::client::AcmeClient;

#[derive(Debug, Deserialize)]
pub struct Order {
    pub status: String,
    pub authorizations: Vec<String>,
    pub finalize: String,
    pub certificate: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Authorization {
    pub status: String,
    pub identifier: Identifier,
    pub challenges: Vec<Challenge>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Identifier {
    #[serde(rename = "type")]
    pub id_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Challenge {
    #[serde(rename = "type")]
    pub challenge_type: String,
    pub url: String,
    #[serde(default)]
    pub token: String,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct OrderMeta {
    pub order_url: String,
}

/// Create a new order for the given domains.
pub async fn create_order(
    client: &AcmeClient,
    domains: &[String],
) -> Result<(String, Order)> {
    let identifiers: Vec<_> = domains
        .iter()
        .map(|d| json!({ "type": "dns", "value": d }))
        .collect();

    let payload = json!({ "identifiers": identifiers });
    let url = client.directory.new_order.clone();
    let resp = client.post(&url, Some(&payload)).await?;

    let order_url = resp
        .headers()
        .get("Location")
        .context("Missing Location header in newOrder response")?
        .to_str()
        .context("Invalid Location header")?
        .to_owned();

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("newOrder failed: {} — {}", status, body);
    }

    let order: Order = serde_json::from_str(&body).context("parse Order")?;
    tracing::info!("Order created: {} (status: {})", order_url, order.status);
    Ok((order_url, order))
}

/// Solve all HTTP-01 challenges in the order, then wait for order to be ready.
/// `challenge_bind`: address for the local challenge server (e.g. "0.0.0.0:5002")
pub async fn solve_challenges(
    client: &AcmeClient,
    order: &Order,
    challenge_bind: &str,
) -> Result<()> {
    let thumbprint = client.account_key.jwk_thumbprint()?;
    let server = ChallengeServer::start(challenge_bind).await?;

    for auth_url in &order.authorizations {
        let resp = client.post_as_get(auth_url).await?;
        let body = resp.text().await?;
        let auth: Authorization = serde_json::from_str(&body).context("parse Authorization")?;

        if auth.status == "valid" {
            tracing::info!("Authorization already valid for {}", auth.identifier.value);
            continue;
        }

        let http_challenge = auth
            .challenges
            .iter()
            .find(|c| c.challenge_type == "http-01")
            .context(format!(
                "No http-01 challenge for {}",
                auth.identifier.value
            ))?;

        let key_auth = key_authorization(&http_challenge.token, &thumbprint);
        server.add_token(http_challenge.token.clone(), key_auth);
        tracing::info!(
            "Registered token for {}",
            auth.identifier.value
        );

        // Notify server the challenge is ready
        client
            .post(&http_challenge.url, Some(&json!({})))
            .await
            .context("notify challenge ready")?;
        tracing::info!("Notified challenge ready for {}", auth.identifier.value);
    }

    // Poll authorizations until all are valid
    for auth_url in &order.authorizations {
        poll_authorization(client, auth_url).await?;
    }

    server.stop();
    Ok(())
}

async fn poll_authorization(client: &AcmeClient, auth_url: &str) -> Result<()> {
    for attempt in 1..=20 {
        sleep(Duration::from_secs(2)).await;
        let resp = client.post_as_get(auth_url).await?;
        let body = resp.text().await?;
        let auth: Authorization = serde_json::from_str(&body).context("parse Authorization poll")?;

        tracing::info!(
            "Authorization poll #{} for {}: {}",
            attempt,
            auth.identifier.value,
            auth.status
        );

        match auth.status.as_str() {
            "valid" => return Ok(()),
            "invalid" => anyhow::bail!(
                "Authorization invalid for {}",
                auth.identifier.value
            ),
            _ => {}
        }
    }
    anyhow::bail!("Authorization did not become valid after polling")
}

/// Finalize the order with the given CSR (DER, base64url encoded).
pub async fn finalize_order(
    client: &AcmeClient,
    order: &Order,
    csr_der_b64: &str,
) -> Result<String> {
    let payload = json!({ "csr": csr_der_b64 });
    let resp = client.post(&order.finalize, Some(&payload)).await?;

    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("finalize failed: {} — {}", status, body);
    }

    // Poll order until valid
    check_order_valid(&body)
}

/// Quick check if the finalize response already has status=valid.
/// Returns the cert URL if valid, or Err to signal the caller should poll by order URL.
fn check_order_valid(initial_body: &str) -> Result<String> {
    let order: Order = serde_json::from_str(initial_body).context("parse Order after finalize")?;
    tracing::info!("Order status after finalize: {}", order.status);
    match order.status.as_str() {
        "valid" => order.certificate.context("Order valid but no certificate URL"),
        "invalid" => anyhow::bail!("Order became invalid"),
        _ => anyhow::bail!("Order not yet valid (status={}), need to poll", order.status),
    }
}

/// Poll an order by its URL until valid, returning the certificate URL.
pub async fn poll_order_by_url(client: &AcmeClient, order_url: &str) -> Result<String> {
    for attempt in 1..=20 {
        sleep(Duration::from_secs(2)).await;
        let resp = client.post_as_get(order_url).await?;
        let body = resp.text().await?;
        let order: Order = serde_json::from_str(&body).context("parse Order")?;

        tracing::info!("Order poll #{}: status={}", attempt, order.status);

        match order.status.as_str() {
            "valid" => {
                return order
                    .certificate
                    .context("Order valid but no certificate URL");
            }
            "invalid" => anyhow::bail!("Order became invalid"),
            _ => {}
        }
    }
    anyhow::bail!("Order did not become valid after polling")
}

/// Download the certificate PEM from the given URL.
pub async fn download_certificate(client: &AcmeClient, cert_url: &str) -> Result<String> {
    let resp = client.post_as_get(cert_url).await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        anyhow::bail!("Failed to download certificate: {} — {}", status, body);
    }
    Ok(body)
}
