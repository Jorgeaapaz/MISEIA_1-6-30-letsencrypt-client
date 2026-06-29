use anyhow::Result;
use axum::{
    extract::{Path, State},
    routing::get,
    Router,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::net::TcpListener;

type TokenMap = Arc<Mutex<HashMap<String, String>>>;

/// Compute the HTTP-01 key authorization: `<token>.<jwk_thumbprint>`
pub fn key_authorization(token: &str, thumbprint: &str) -> String {
    format!("{}.{}", token, thumbprint)
}

/// Start a temporary HTTP server on `bind_addr` (e.g. "0.0.0.0:5002") that
/// serves ACME HTTP-01 challenges. Returns (server handle, token map).
/// The caller inserts tokens via `add_token`, then drops the handle to stop.
pub struct ChallengeServer {
    pub tokens: TokenMap,
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
}

impl ChallengeServer {
    pub async fn start(bind_addr: &str) -> Result<Self> {
        let tokens: TokenMap = Arc::new(Mutex::new(HashMap::new()));
        let tokens_clone = tokens.clone();

        let app = Router::new()
            .route("/.well-known/acme-challenge/:token", get(serve_challenge))
            .with_state(tokens_clone);

        let addr: SocketAddr = bind_addr.parse()?;
        let listener = TcpListener::bind(addr).await?;
        tracing::info!("Challenge server listening on {}", addr);

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .ok();
        });

        Ok(Self {
            tokens,
            shutdown_tx,
        })
    }

    pub fn add_token(&self, token: String, key_auth: String) {
        // Mutex poison only occurs if another thread panicked while holding the lock,
        // which cannot happen in this single-writer flow.
        #[allow(clippy::unwrap_used)]
        self.tokens.lock().unwrap().insert(token, key_auth);
    }

    pub fn stop(self) {
        let _ = self.shutdown_tx.send(());
        tracing::info!("Challenge server stopped");
    }
}

async fn serve_challenge(
    Path(token): Path<String>,
    State(tokens): State<TokenMap>,
) -> axum::response::Response<String> {
    // Mutex poison only occurs if a thread panicked while holding the lock —
    // impossible here since add_token is the only writer and it cannot panic.
    #[allow(clippy::unwrap_used)]
    let map = tokens.lock().unwrap();
    if let Some(key_auth) = map.get(&token) {
        tracing::debug!("Serving challenge token: {}", token);
        // Response::builder() only errors on invalid status/headers, which are
        // hard-coded constants here — the unwrap is infallible.
        #[allow(clippy::unwrap_used)]
        axum::response::Response::builder()
            .status(200)
            .header("Content-Type", "application/octet-stream")
            .body(key_auth.clone())
            .unwrap()
    } else {
        tracing::warn!("Unknown challenge token: {}", token);
        #[allow(clippy::unwrap_used)]
        axum::response::Response::builder()
            .status(404)
            .body(String::new())
            .unwrap()
    }
}
