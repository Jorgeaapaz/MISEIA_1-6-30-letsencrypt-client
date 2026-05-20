mod acme;
mod cert;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use acme::{
    account::ensure_account,
    client::AcmeClient,
    crypto::AccountKey,
    order::{create_order, download_certificate, finalize_order, poll_order_by_url, solve_challenges},
};
use cert::{
    csr::generate_csr,
    storage::{save_certificate, show_certificate},
};

#[derive(Parser)]
#[command(name = "acme-client", version, about = "ACME (RFC 8555) certificate client")]
struct Cli {
    /// ACME directory URL
    #[arg(
        long,
        global = true,
        default_value = "https://acme-v02.api.letsencrypt.org/directory",
        env = "ACME_URL"
    )]
    acme_url: String,

    /// Output directory for certificates
    #[arg(long, global = true, default_value = "./certs", env = "ACME_OUTPUT")]
    output: PathBuf,

    /// Disable TLS certificate verification (for Pebble)
    #[arg(long, global = true, default_value = "false", env = "ACME_INSECURE_TLS")]
    insecure: bool,

    /// Challenge server bind address (host:port)
    #[arg(long, global = true, default_value = "0.0.0.0:5002")]
    challenge_bind: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Obtain a new certificate for one or more domains
    Issue {
        /// Domains to include (first is CN, all go into SAN)
        #[arg(long = "domain", required = true)]
        domains: Vec<String>,

        /// Contact email for the ACME account
        #[arg(long, default_value = "admin@example.com")]
        email: String,
    },

    /// Renew a certificate (re-issue)
    Renew {
        /// Primary domain whose certificate to renew
        #[arg(long = "domain", required = true)]
        domains: Vec<String>,

        #[arg(long, default_value = "admin@example.com")]
        email: String,
    },

    /// Show stored certificate info
    Show {
        /// Domain to inspect
        #[arg(long)]
        domain: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("acme_client=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Issue { domains, email } | Command::Renew { domains, email } => {
            issue_certificate(
                &cli.acme_url,
                &domains,
                &email,
                &cli.output,
                cli.insecure,
                &cli.challenge_bind,
            )
            .await?;
        }
        Command::Show { domain } => {
            show_certificate(&cli.output, &domain)?;
        }
    }

    Ok(())
}

async fn issue_certificate(
    acme_url: &str,
    domains: &[String],
    email: &str,
    output: &PathBuf,
    insecure: bool,
    challenge_bind: &str,
) -> Result<()> {
    let accounts_dir = output.join(".accounts");

    // Step 1: Generate a fresh key (ensure_account will replace it if one exists on disk)
    let account_key = AccountKey::generate()?;

    // Step 2: Build ACME client
    let mut client = AcmeClient::new(acme_url, account_key, insecure).await?;

    // Step 3: Create or load account
    ensure_account(&mut client, email, &accounts_dir).await?;

    // Step 4: Create order
    let (order_url, order) = create_order(&client, domains).await?;

    // Step 5 & 6: Solve HTTP-01 challenges
    solve_challenges(&client, &order, challenge_bind).await?;

    // Step 7: Generate CSR
    let csr = generate_csr(domains)?;

    // Step 8: Finalize order
    tracing::info!("Finalizing order...");
    let cert_url = match finalize_order(&client, &order, &csr.csr_b64).await {
        Ok(url) => url,
        Err(_) => {
            // Fallback: poll by order URL
            poll_order_by_url(&client, &order_url).await?
        }
    };

    // Step 9: Download certificate
    tracing::info!("Downloading certificate from {}", cert_url);
    let cert_pem = download_certificate(&client, &cert_url).await?;

    // Step 10: Save to disk
    let primary_domain = &domains[0];
    let paths = save_certificate(output, primary_domain, &csr.private_key_pem, &cert_pem)?;

    println!("Certificate issued successfully!");
    println!("  Private key : {:?}", paths.privkey);
    println!("  Certificate : {:?}", paths.cert);
    println!("  Full chain  : {:?}", paths.fullchain);

    Ok(())
}
