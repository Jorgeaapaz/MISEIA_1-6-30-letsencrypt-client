# Run, Test and Stop — ACME Client (Rust)

Complete step-by-step guide to build, run, test, and stop this project on **Windows 11** using the Pebble test CA.

---

## Table of Contents

1. [Project Architecture](#1-project-architecture)
2. [Prerequisites](#2-prerequisites)
3. [One-Time Setup](#3-one-time-setup)
4. [Start the Environment](#4-start-the-environment)
5. [Build the Rust Client](#5-build-the-rust-client)
6. [Fetch the Pebble CA Certificate](#6-fetch-the-pebble-ca-certificate)
7. [Testing Existing Certificates](#7-testing-existing-certificates)
8. [Simple Domain Cert — test3.example.com](#8-simple-domain-cert--test3examplecom)
9. [Multi-Domain Cert — test4 + www.test4](#9-multi-domain-cert--test4examplecom--wwwtest4examplecom)
10. [Understanding the Certificate Files](#10-understanding-the-certificate-files)
11. [Verify with the Express Test App](#11-verify-with-the-express-test-app)
12. [Inspect a Certificate](#12-inspect-a-certificate)
13. [Stop Everything](#13-stop-everything)

---

## 1. Project Architecture

### 1.1 Rust ACME Client (`src/`)

The core of this project is a CLI tool written in Rust that implements the **ACME protocol (RFC 8555)** — the same protocol that Let's Encrypt uses to issue TLS certificates automatically.

**What it does end-to-end:**

```
You run: acme-client issue --domain example.com
           │
           ▼
1. Fetch ACME Directory        GET  /directory  → get endpoint URLs
2. Get Nonce                   HEAD /newNonce   → anti-replay token
3. Create/Load Account         POST /newAccount → register JWK + accept ToS
           │
           ▼
4. Create Order                POST /newOrder   → "I want a cert for example.com"
5. Fetch Authorization         GET  /authz/...  → get challenge details
6. Solve HTTP-01 Challenge     spawn axum HTTP server on :5002
                               serve /.well-known/acme-challenge/<token>
7. Notify Challenge Ready      POST /chall/...  → "challenge is up, please verify"
8. Poll Authorization          GET  /authz/...  → wait for "valid"
           │
           ▼
9.  Generate CSR               rcgen creates new key pair + CSR
10. Finalize Order             POST /finalize   → submit CSR DER (base64url)
11. Poll Order                 GET  /order/...  → wait for "valid"
12. Download Certificate       GET  /cert/...   → PEM certificate chain
13. Save to disk               certs/<domain>/{privkey,cert,fullchain}.pem
```

**Module breakdown:**

| File | Responsibility |
|------|---------------|
| `src/main.rs` | CLI entrypoint (clap). Parses `issue`, `renew`, `show` subcommands. Orchestrates the full flow. |
| `src/acme/directory.rs` | Fetches the ACME directory JSON and deserializes endpoint URLs. |
| `src/acme/client.rs` | `AcmeClient` struct. Wraps `reqwest` HTTP client. Handles JWS-signed POST and POST-as-GET requests, fetches fresh nonces on every request. |
| `src/acme/crypto.rs` | `AccountKey`: generates ECDSA P-256 key pairs using `ring`. Builds JWK, computes JWK thumbprint (SHA-256), signs JWS envelopes (ES256). |
| `src/acme/account.rs` | Creates or loads an ACME account. Persists the account URL and private key to `certs/.accounts/account.json`. |
| `src/acme/order.rs` | Creates orders, fetches authorizations, polls order/auth status, finalizes the order with the CSR, downloads the certificate. |
| `src/acme/challenge.rs` | Spawns a temporary `axum` HTTP server on port 5002. Registers `token → key_authorization` pairs in memory. Serves `/.well-known/acme-challenge/<token>`. Stops when challenge is complete. |
| `src/cert/csr.rs` | Generates a fresh certificate key pair and a DER-encoded CSR using `rcgen`. First domain = CN; all domains → SAN. |
| `src/cert/storage.rs` | Writes `privkey.pem`, `cert.pem` (leaf only), `fullchain.pem` (full chain) to `certs/<domain>/`. Also implements `show` — parses and displays X.509 metadata. |

**Key cryptography decisions:**

- **Account key** (signs ACME requests): ECDSA P-256 via `ring`, persisted as PKCS#8 DER.
- **Certificate key** (in the issued TLS cert): generated fresh per `issue` run by `rcgen`.
- **JWS** (JSON Web Signature): built manually — base64url(header) + "." + base64url(payload), signed with ECDSA P-256 (ES256).
- **JWK thumbprint**: canonical JSON of the public key (sorted keys `crv`, `kty`, `x`, `y`) hashed with SHA-256.

---

### 1.2 Docker Pebble (`docker-compose.yml`)

**Pebble** is Let's Encrypt's lightweight ACME test server. It behaves identically to production Let's Encrypt but:
- Uses a self-signed root CA (you must supply `--cacert` to curl).
- Does not require real DNS — it resolves challenge servers via `extra_hosts` (Docker's host-gateway).
- Validates HTTP-01 challenges by connecting to the host machine on port **5002** (configured in `docker/pebble-config.json`).
- Intentionally introduces random delays and nonce rejections to stress-test clients (disabled here via `PEBBLE_VA_NOSLEEP=1` and `PEBBLE_WFE_NONCEREJECT=0`).

**Services started by `docker compose up`:**

| Service | Image | Ports | Purpose |
|---------|-------|-------|---------|
| `pebble` | `ghcr.io/letsencrypt/pebble:latest` | `14000` (ACME HTTPS), `15000` (management HTTP) | ACME CA server |

**Why `extra_hosts`?** When Pebble validates an HTTP-01 challenge, it opens a TCP connection to the domain being validated. On Docker, the domain (`test3.example.com`) would normally not resolve. The `extra_hosts` entries map each test domain to `host-gateway` (the Docker host IP), so Pebble's validation requests reach the `axum` challenge server running on your machine at port 5002.

**Pebble management API** (port 15000, HTTP, no auth):
- `GET /roots/0` — Root CA certificate (PEM)
- `GET /intermediates/0` — Intermediate CA certificate (PEM)

---

### 1.3 Express Test App (`test-app/`)

A minimal **Node.js / Express** HTTPS server that proves the issued certificate works in a real TLS stack.

**What it does:**
- Reads `certs/<DOMAIN>/privkey.pem` and `certs/<DOMAIN>/fullchain.pem` at startup.
- Starts an `https.createServer()` on port **8443**.
- Exposes two routes:
  - `GET /` — returns JSON with domain, issuer, validity dates, and SAN list.
  - `GET /health` — returns `{ status: "ok", timestamp }`.

**Configuration (environment variables):**

| Variable | Default | Description |
|----------|---------|-------------|
| `DOMAIN` | `test1.example.com` | Which cert to load from `certs/` |
| `PORT` | `8443` | HTTPS listen port |
| `CERTS_DIR` | `../certs` | Root directory for certificates |

The app is purely a validator — it confirms that the cert files are valid PEM, that Node's TLS stack accepts them, and that curl with the Pebble CA can complete the TLS handshake.

---

## 2. Prerequisites

Install the following before continuing. Verify each with the command shown.

| Tool | Where to install | Verify |
|------|-----------------|--------|
| **Rust + Cargo** | https://rustup.rs | `cargo --version` |
| **Docker Desktop** | https://www.docker.com/products/docker-desktop/ | `docker --version` |
| **Node.js 18+** | https://nodejs.org | `node --version` |
| **Git for Windows** | https://git-scm.com/download/win | `git --version` |
| **curl** | bundled with Git Bash | `curl --version` |

> **Terminal legend used in this guide:**
> - **PowerShell** — Windows PowerShell or PowerShell 7 (`pwsh`). Run from the project root.
> - **Git Bash** — The bash shell that ships with Git for Windows. Run from the project root.
> - **Elevated PowerShell** — PowerShell started with "Run as Administrator". Only needed for hosts file edits.

All commands below assume your working directory is the project root:
```
D:\Master-IA-Dev\06-Bloque6\1-6-30-letsencrypt-client\
```

---

## 3. One-Time Setup

### 3.1 Add Test Domains to Windows Hosts File

> **Terminal: Elevated PowerShell** (Run as Administrator)

Open PowerShell as Administrator, then run:

```powershell
$hostsFile = "C:\Windows\System32\drivers\etc\hosts"
$entries = @(
    "127.0.0.1  test1.example.com",
    "127.0.0.1  test2.example.com",
    "127.0.0.1  www.test1.example.com",
    "127.0.0.1  www.test2.example.com",
    "127.0.0.1  test3.example.com",
    "127.0.0.1  test4.example.com",
    "127.0.0.1  www.test4.example.com"
)
$current = Get-Content $hostsFile -Raw
foreach ($entry in $entries) {
    $domain = ($entry -split "\s+")[1]
    if ($current -notmatch [regex]::Escape($domain)) {
        Add-Content $hostsFile "`n$entry"
        Write-Host "Added: $domain"
    } else {
        Write-Host "Already present: $domain"
    }
}
Write-Host "Done."
```

Verify:
```powershell
ping -n 1 test3.example.com
ping -n 1 test4.example.com
ping -n 1 www.test4.example.com
```
You should see replies from `127.0.0.1`.

---

### 3.2 Install Node.js Dependencies for the Test App

> **Terminal: PowerShell**

```powershell
cd test-app
npm install
cd ..
```

---

## 4. Start the Environment

### 4.1 Start Pebble (Docker)

> **Terminal: PowerShell**

```powershell
docker compose up -d
```

Wait ~5 seconds for Pebble to initialize, then verify it is up:

```powershell
docker compose ps
```

Expected output:
```
NAME      IMAGE                              STATUS
pebble    ghcr.io/letsencrypt/pebble:latest  Up
```

Check Pebble logs (optional):
```powershell
docker compose logs pebble
```

You should see a line like:
```
Listening on: 0.0.0.0:14000
```

---

## 5. Build the Rust Client

> **Terminal: PowerShell**

```powershell
cargo build --release
```

First build downloads all crates and may take 2–5 minutes. Subsequent builds are incremental.

Verify the binary exists:
```powershell
.\target\release\acme-client.exe --version
```

Expected output:
```
acme-client 0.1.0
```

---

## 6. Fetch the Pebble CA Certificate

Pebble uses a self-signed root CA. You must fetch it once so that `curl` can verify the certificates you issue.

> **Terminal: Git Bash**

```bash
bash scripts/fetch-pebble-ca.sh
```

This creates `docker/pebble-root-ca.pem` (root + intermediate CA concatenated).

Alternatively, run manually in **Git Bash**:
```bash
curl -sk https://localhost:15000/roots/0 > docker/pebble-root-ca.pem
curl -sk https://localhost:15000/intermediates/0 >> docker/pebble-root-ca.pem
echo "Pebble CA saved to docker/pebble-root-ca.pem"
```

Verify the file was created:
```bash
ls -lh docker/pebble-root-ca.pem
```

---

## 7. Testing Existing Certificates

You already have certificates on disk for `test1.example.com` (simple) and `test2.example.com` / `www.test2.example.com` (multi-domain). This section shows how to inspect them and serve them with the Express app — **no new issuance needed**.

> **Requirement:** Pebble must be running (`docker compose up -d`) because the Express app only needs the cert files already on disk, but `curl` needs the Pebble CA to verify the TLS chain.
> Also ensure the Pebble CA has been fetched: `docker/pebble-root-ca.pem` must exist (see [section 6](#6-fetch-the-pebble-ca-certificate)).

---

### 7.1 Inspect Existing Certificates (no server needed)

> **Terminal: PowerShell**

```powershell
# Simple cert — test1.example.com
.\target\release\acme-client.exe --output ./certs show --domain test1.example.com

# Multi-domain cert — test2.example.com (SAN includes www.test2.example.com)
.\target\release\acme-client.exe --output ./certs show --domain test2.example.com
```

Expected output for `test1.example.com`:
```
Domain       : test1.example.com
Subject      : CN=test1.example.com
Issuer       : CN=Pebble Intermediate CA ...
Not Before   : ...
Not After    : ...
SANs         : test1.example.com
```

Expected output for `test2.example.com`:
```
Domain       : test2.example.com
Subject      : CN=test2.example.com
Issuer       : CN=Pebble Intermediate CA ...
Not Before   : ...
Not After    : ...
SANs         : test2.example.com, www.test2.example.com
```

---

### 7.2 Serve test1.example.com via HTTPS

> **Terminal: PowerShell (new window)**

```powershell
$env:DOMAIN   = "test1.example.com"
$env:PORT     = "8443"
$env:CERTS_DIR = "$PWD\certs"
node test-app\server.js
```

> **Terminal: Git Bash**

```bash
# Root endpoint
curl --cacert docker/pebble-root-ca.pem https://test1.example.com:8443/

# Health check
curl --cacert docker/pebble-root-ca.pem https://test1.example.com:8443/health
```

Expected `/` response:
```json
{
  "domain": "test1.example.com",
  "issued_by": "CN=Pebble Intermediate CA ...",
  "valid_from": "...",
  "valid_until": "...",
  "san": "DNS:test1.example.com",
  "host": "test1.example.com:8443"
}
```

Stop the server with **Ctrl+C** before starting the next one (both use port 8443).

---

### 7.3 Serve test2.example.com via HTTPS (Multi-Domain)

> **Terminal: PowerShell (new window)**

```powershell
$env:DOMAIN   = "test2.example.com"
$env:PORT     = "8443"
$env:CERTS_DIR = "$PWD\certs"
node test-app\server.js
```

> **Terminal: Git Bash**

```bash
# Primary domain
curl --cacert docker/pebble-root-ca.pem https://test2.example.com:8443/

# Alternate name covered by the same certificate
curl --cacert docker/pebble-root-ca.pem https://www.test2.example.com:8443/
```

Both requests use the same certificate (stored under `test2.example.com`). The TLS handshake succeeds for `www.test2.example.com` because that name is listed in the SAN extension.

Expected `/` response for `www.test2.example.com`:
```json
{
  "domain": "test2.example.com",
  "issued_by": "CN=Pebble Intermediate CA ...",
  "san": "DNS:test2.example.com, DNS:www.test2.example.com",
  "host": "www.test2.example.com:8443"
}
```

Stop the server with **Ctrl+C**.

---

### 7.4 Verify Cert Files on Disk

> **Terminal: PowerShell**

```powershell
# List all issued certs
Get-ChildItem -Path certs -Recurse -Filter "*.pem" | Select-Object FullName, Length

# Quick check — cert1 subject line
Get-Content certs\test1.example.com\cert.pem | Select-String "BEGIN"

# Check test2 fullchain covers both domains (look for both SAN entries)
# (use openssl if available in Git Bash)
```

> **Terminal: Git Bash** (requires openssl, bundled with Git for Windows)

```bash
# Decode and print cert details for test1
openssl x509 -in certs/test1.example.com/cert.pem -noout -subject -issuer -dates

# Decode and print SANs for test2 (should list both domains)
openssl x509 -in certs/test2.example.com/cert.pem -noout -ext subjectAltName
```

---

## 8. Simple Domain Cert — test3.example.com

This issues a certificate for a **single domain**.

> **Terminal: PowerShell**

```powershell
.\target\release\acme-client.exe `
  --acme-url https://localhost:14000/dir `
  --insecure `
  --output ./certs `
  issue `
  --domain test3.example.com `
  --email admin@example.com
```

### What happens step by step:

1. The client fetches the Pebble directory at `https://localhost:14000/dir`.
2. Creates (or loads) an ACME account and saves it to `certs/.accounts/account.json`.
3. Creates an order for `test3.example.com`.
4. Fetches the HTTP-01 challenge token from Pebble.
5. Spawns an axum HTTP server on `0.0.0.0:5002` serving the challenge token.
6. Notifies Pebble that the challenge is ready.
7. Pebble connects to `test3.example.com:5002` (resolved via Docker `extra_hosts` → host-gateway → your machine) and fetches `/.well-known/acme-challenge/<token>`.
8. Authorization becomes `valid`. Challenge server stops.
9. Generates a new ECDSA key pair and CSR for `test3.example.com`.
10. Finalizes the order by sending the CSR. Pebble issues the certificate.
11. Downloads and saves the certificate chain.

### Expected output:

```
Certificate issued successfully!
  Private key : "certs\\test3.example.com\\privkey.pem"
  Certificate : "certs\\test3.example.com\\cert.pem"
  Full chain  : "certs\\test3.example.com\\fullchain.pem"
```

### Verify the files:

```powershell
dir certs\test3.example.com\
```

```
privkey.pem      # Certificate private key (ECDSA P-256, PEM)
cert.pem         # Leaf certificate only (PEM)
fullchain.pem    # Leaf + intermediate chain (PEM)
```

### Inspect the certificate:

```powershell
.\target\release\acme-client.exe --output ./certs show --domain test3.example.com
```

---

## 9. Multi-Domain Cert — test4.example.com + www.test4.example.com

This issues a **single certificate covering two domains** (multi-SAN). The first domain becomes the CN; all domains appear in the Subject Alternative Names.

> **Terminal: PowerShell**

```powershell
.\target\release\acme-client.exe `
  --acme-url https://localhost:14000/dir `
  --insecure `
  --output ./certs `
  issue `
  --domain test4.example.com `
  --domain www.test4.example.com `
  --email admin@example.com
```

### What is different from the simple cert:

- The order contains **two identifiers** (`test4.example.com` and `www.test4.example.com`).
- Pebble returns **two authorization objects**, one per domain.
- The challenge server registers **two tokens** simultaneously (one per domain).
- Both domains are validated before the order moves to `ready`.
- The CSR contains **both domains in the SAN extension**.
- The resulting certificate is stored under the **primary domain** (`test4.example.com`).

### Expected output:

```
Certificate issued successfully!
  Private key : "certs\\test4.example.com\\privkey.pem"
  Certificate : "certs\\test4.example.com\\cert.pem"
  Full chain  : "certs\\test4.example.com\\fullchain.pem"
```

### Inspect the multi-domain certificate:

```powershell
.\target\release\acme-client.exe --output ./certs show --domain test4.example.com
```

Expected output will show both SANs:
```
Domain       : test4.example.com
Subject      : CN=test4.example.com
Issuer       : ...Pebble...
Not Before   : ...
Not After    : ...
SANs         : test4.example.com, www.test4.example.com
```

---

## 10. Understanding the Certificate Files

After a successful `acme-client issue`, three PEM files appear under `certs/<domain>/`. Each one has a distinct role and is consumed by different tools.

```
certs/test3.example.com/
├── privkey.pem
├── cert.pem
└── fullchain.pem
```

---

### 10.1 `privkey.pem` — Certificate Private Key

```
-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQg...
-----END PRIVATE KEY-----
```

**What it is:** The private half of the ECDSA P-256 key pair generated by `rcgen` at issuance time. This key matches the public key embedded inside `cert.pem`.

**Important distinctions:**
- This is the **certificate key** — it identifies your server during TLS handshakes.
- It is **completely separate** from the ACME account key stored in `certs/.accounts/account.json`. The account key signs ACME protocol messages; the certificate key is what your web server uses.
- A new key pair is generated on every `issue` run. Renewing does not reuse the old key.

**Who uses it:** Every TLS server that presents the certificate — nginx, Node.js `https.createServer`, Rust `rustls`, etc. The server needs this to prove during the TLS handshake that it owns the certificate.

**Security:** Keep this file private. Never commit it to git (it is already covered by `.gitignore`). If it leaks, an attacker can impersonate your server until the certificate expires.

---

### 10.2 `cert.pem` — Leaf Certificate Only

```
-----BEGIN CERTIFICATE-----
MIICpDCCAYwCFDkq8s... (single certificate block)
-----END CERTIFICATE-----
```

**What it is:** The X.509 v3 end-entity certificate issued and signed by the Pebble Intermediate CA. It contains:

| Field | Value (example) |
|-------|----------------|
| **Subject** | `CN=test3.example.com` |
| **Issuer** | `CN=Pebble Intermediate CA ...` |
| **Public Key** | ECDSA P-256 (matches `privkey.pem`) |
| **SANs** | `DNS:test3.example.com` (or multiple for multi-domain) |
| **Validity** | `Not Before` / `Not After` — the certificate's lifetime |
| **Serial** | Unique identifier assigned by the CA |

**Who uses it:** Tools that need to inspect or display just the leaf certificate, such as `acme-client show`, `openssl x509 -in cert.pem -text`, or monitoring systems that check expiry dates. It is **not** what you hand to a TLS server, because it is incomplete — it omits the intermediate CA certificate that browsers and clients need to build the trust chain.

**Why it exists separately:** Some tools (e.g., OCSP stapling checks, certificate transparency monitors) need just the leaf, not the full chain.

---

### 10.3 `fullchain.pem` — Full Certificate Chain

```
-----BEGIN CERTIFICATE-----
MIICpDCCAYwCFDkq8s... (leaf certificate)
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
MIICpTCCAYsCFHjY... (Pebble Intermediate CA)
-----END CERTIFICATE-----
```

**What it is:** The leaf certificate concatenated with every intermediate CA certificate up to (but not including) the root CA. This is the file you give to your TLS server.

**The chain of trust:**

```
Root CA (self-signed, in docker/pebble-root-ca.pem)
    └── Intermediate CA  ← second block in fullchain.pem
            └── Your cert (test3.example.com)  ← first block in fullchain.pem
```

When a client (curl, browser) connects:
1. The server sends `fullchain.pem` during the TLS handshake.
2. The client reads the intermediate CA block and checks it is signed by a CA it trusts.
3. The client checks the root CA against its trust store (or `--cacert` in curl).
4. The chain validates → TLS handshake succeeds.

If you serve only `cert.pem` (leaf alone), clients that do not already have the intermediate CA cached will fail with `unable to get local issuer certificate`.

**Who uses it:** Every TLS server. The Express test app loads `fullchain.pem` as the `cert` option passed to `https.createServer()`. nginx calls this `ssl_certificate`. Let's Encrypt's own documentation always recommends `fullchain.pem` over `cert.pem` for server configuration.

---

### 10.4 Summary Table

| File | Contains | Used by |
|------|----------|---------|
| `privkey.pem` | Your server's private key | TLS server (Node, nginx, rustls…) |
| `cert.pem` | Leaf certificate only | Inspection tools, monitoring |
| `fullchain.pem` | Leaf + intermediate chain | TLS server — **use this for `cert` config** |

> **Rule of thumb:** Configure your TLS server with **`fullchain.pem`** (not `cert.pem`) and **`privkey.pem`**. Use `cert.pem` only when you need to inspect or parse just the leaf certificate.

---

### 10.5 Inspect the Files with OpenSSL

> **Terminal: Git Bash** (`openssl` is bundled with Git for Windows)

```bash
# Print subject, issuer, and validity dates from the leaf cert
openssl x509 -in certs/test3.example.com/cert.pem -noout -subject -issuer -dates

# Print all SANs (critical for multi-domain certs)
openssl x509 -in certs/test4.example.com/cert.pem -noout -ext subjectAltName

# Show how many certificates are in the fullchain
grep -c "BEGIN CERTIFICATE" certs/test3.example.com/fullchain.pem

# Print the public key from the leaf cert (should match privkey.pem)
openssl x509 -in certs/test3.example.com/cert.pem -noout -pubkey

# Print the public key from the private key (must be identical to the line above)
openssl pkey -in certs/test3.example.com/privkey.pem -pubout

# Verify the private key and the certificate are a matching pair
diff \
  <(openssl x509 -in certs/test3.example.com/cert.pem  -noout -pubkey) \
  <(openssl pkey  -in certs/test3.example.com/privkey.pem -pubout) \
  && echo "Keys match" || echo "MISMATCH — something is wrong"
```

---

## 11. Verify with the Express Test App

The Express app loads a certificate from `certs/<DOMAIN>/` and starts an HTTPS server. Use it to confirm the full TLS handshake works.

### 11.1 Test with test3.example.com (Simple Cert)

Open a **new PowerShell window** and start the server:

> **Terminal: PowerShell (new window)**

```powershell
$env:DOMAIN = "test3.example.com"
$env:PORT = "8443"
$env:CERTS_DIR = "$PWD\certs"
node test-app\server.js
```

Expected console output:
```
HTTPS server running at https://test3.example.com:8443
Test with:
  curl --cacert ../docker/pebble-root-ca.pem https://test3.example.com:8443/
```

In a **second Git Bash window**, test the endpoints:

> **Terminal: Git Bash**

```bash
# Main endpoint
curl --cacert docker/pebble-root-ca.pem https://test3.example.com:8443/

# Health check
curl --cacert docker/pebble-root-ca.pem https://test3.example.com:8443/health
```

Expected response for `/`:
```json
{
  "domain": "test3.example.com",
  "issued_by": "CN=Pebble Intermediate CA ...",
  "valid_from": "...",
  "valid_until": "...",
  "san": "DNS:test3.example.com",
  "host": "test3.example.com:8443"
}
```

Expected response for `/health`:
```json
{ "status": "ok", "timestamp": "2026-06-09T..." }
```

Stop the server with **Ctrl+C** in the PowerShell window.

---

### 11.2 Test with test4.example.com (Multi-Domain Cert)

> **Terminal: PowerShell (new window)**

```powershell
$env:DOMAIN = "test4.example.com"
$env:PORT = "8443"
$env:CERTS_DIR = "$PWD\certs"
node test-app\server.js
```

> **Terminal: Git Bash**

```bash
# Test primary domain
curl --cacert docker/pebble-root-ca.pem https://test4.example.com:8443/

# Test alternate name (same cert, same server)
curl --cacert docker/pebble-root-ca.pem https://www.test4.example.com:8443/
```

Both requests should succeed. The response for `www.test4.example.com` will confirm `san` contains both names:
```json
{
  "domain": "test4.example.com",
  "san": "DNS:test4.example.com, DNS:www.test4.example.com",
  ...
}
```

Stop the server with **Ctrl+C**.

---

### 11.3 Confirm TLS Verification Fails Without the CA

This confirms your cert is properly signed — not just self-signed:

> **Terminal: Git Bash**

```bash
# Should FAIL — curl cannot verify Pebble's CA by default
curl https://test3.example.com:8443/
```

Expected error: `SSL certificate problem: unable to get local issuer certificate`

---

## 12. Inspect a Certificate

The `show` subcommand reads the stored `cert.pem` and prints X.509 metadata without making any network calls.

> **Terminal: PowerShell**

```powershell
# Show simple cert
.\target\release\acme-client.exe --output ./certs show --domain test3.example.com

# Show multi-domain cert
.\target\release\acme-client.exe --output ./certs show --domain test4.example.com
```

---

## 13. Stop Everything

### 13.1 Stop the Express Test App

In the PowerShell window running `node test-app\server.js`, press:
```
Ctrl+C
```

### 13.2 Stop and Remove Pebble Containers

> **Terminal: PowerShell**

```powershell
# Stop containers (keeps the image cached)
docker compose down
```

To also remove the downloaded image:
```powershell
docker compose down --rmi all
```

Verify everything is stopped:
```powershell
docker compose ps
```

Expected output: empty (no running services).

---

## Quick Reference

### Pebble Endpoints

| URL | Description |
|-----|-------------|
| `https://localhost:14000/dir` | ACME directory (use `--insecure`) |
| `http://localhost:15000/roots/0` | Pebble Root CA (PEM) |
| `http://localhost:15000/intermediates/0` | Pebble Intermediate CA (PEM) |

### acme-client Command Reference

```
# Issue simple cert
.\target\release\acme-client.exe --acme-url https://localhost:14000/dir --insecure issue --domain <domain> --email <email>

# Issue multi-domain cert
.\target\release\acme-client.exe --acme-url https://localhost:14000/dir --insecure issue --domain <d1> --domain <d2> --email <email>

# Renew (same as issue, overwrites existing cert)
.\target\release\acme-client.exe --acme-url https://localhost:14000/dir --insecure renew --domain <domain> --email <email>

# Show stored cert metadata
.\target\release\acme-client.exe --output ./certs show --domain <domain>
```

### Environment Variables (alternative to flags)

| Variable | Flag equivalent | Example |
|----------|----------------|---------|
| `ACME_URL` | `--acme-url` | `https://localhost:14000/dir` |
| `ACME_INSECURE_TLS` | `--insecure` | `true` |
| `ACME_OUTPUT` | `--output` | `./certs` |

Set in PowerShell:
```powershell
$env:ACME_URL = "https://localhost:14000/dir"
$env:ACME_INSECURE_TLS = "true"
.\target\release\acme-client.exe issue --domain test3.example.com
```

### Certificate File Layout

```
certs/
├── .accounts/
│   └── account.json          # ACME account URL + private key (hex PKCS8)
├── test3.example.com/
│   ├── privkey.pem           # Certificate private key
│   ├── cert.pem              # Leaf certificate only
│   └── fullchain.pem         # Leaf + intermediate chain
└── test4.example.com/
    ├── privkey.pem
    ├── cert.pem
    └── fullchain.pem         # SAN: test4.example.com, www.test4.example.com
```

---

## Troubleshooting

**Port 5002 already in use**
```powershell
netstat -ano | findstr :5002
# Kill the process using that PID:
Stop-Process -Id <PID> -Force
```

**Pebble cannot reach the challenge server**
Confirm Docker `extra_hosts` includes your domain. Re-run `docker compose down` then `docker compose up -d` after editing `docker-compose.yml`.

**`cargo build` fails on Windows**
Ensure the MSVC build tools are installed. Run `rustup show` and confirm the `stable-x86_64-pc-windows-msvc` toolchain is active.

**`curl` not found in PowerShell**
Use Git Bash for `curl` commands, or install curl separately. PowerShell's built-in `curl` is an alias for `Invoke-WebRequest` and does not support `--cacert`.

**Cert missing for test-app startup**
Run `acme-client issue` for the domain first. The app exits immediately if cert files are missing.
