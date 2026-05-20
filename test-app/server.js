'use strict';

const https = require('https');
const fs = require('fs');
const path = require('path');
const express = require('express');
const crypto = require('crypto');

// ─── Configuration ────────────────────────────────────────────────────────────
const DOMAIN = process.env.DOMAIN || 'test1.example.com';
const PORT = parseInt(process.env.PORT || '8443', 10);
const CERTS_DIR = process.env.CERTS_DIR || path.join(__dirname, '..', 'certs');

const certDir = path.join(CERTS_DIR, DOMAIN);
const keyPath = path.join(certDir, 'privkey.pem');
const certPath = path.join(certDir, 'fullchain.pem');

// ─── Load certificates ────────────────────────────────────────────────────────
function loadCerts() {
  if (!fs.existsSync(keyPath) || !fs.existsSync(certPath)) {
    console.error(`Missing certificates for domain '${DOMAIN}'.`);
    console.error(`Expected at: ${certDir}`);
    console.error(`Run: acme-client issue --domain ${DOMAIN}`);
    process.exit(1);
  }
  return {
    key: fs.readFileSync(keyPath, 'utf8'),
    cert: fs.readFileSync(certPath, 'utf8'),
  };
}

// ─── Parse certificate metadata ───────────────────────────────────────────────
function parseCertInfo(certPem) {
  try {
    const cert = new crypto.X509Certificate(certPem);
    return {
      subject: cert.subject,
      issuer: cert.issuer,
      validFrom: cert.validFrom,
      validTo: cert.validTo,
      subjectAltName: cert.subjectAltName,
    };
  } catch {
    return null;
  }
}

// ─── Express app ──────────────────────────────────────────────────────────────
const app = express();

app.get('/health', (_req, res) => {
  res.json({ status: 'ok', timestamp: new Date().toISOString() });
});

app.get('/', (req, res) => {
  const creds = loadCerts();
  const info = parseCertInfo(creds.cert);

  res.json({
    domain: DOMAIN,
    issued_by: info?.issuer ?? 'unknown',
    valid_from: info?.validFrom ?? null,
    valid_until: info?.validTo ?? null,
    san: info?.subjectAltName ?? null,
    host: req.headers.host,
  });
});

// ─── Start HTTPS server ───────────────────────────────────────────────────────
const creds = loadCerts();
const server = https.createServer(creds, app);

server.listen(PORT, () => {
  console.log(`HTTPS server running at https://${DOMAIN}:${PORT}`);
  console.log(`Test with:`);
  console.log(`  curl --cacert ../docker/pebble-root-ca.pem https://${DOMAIN}:${PORT}/`);
  console.log(`  curl --cacert ../docker/pebble-root-ca.pem https://${DOMAIN}:${PORT}/health`);
});
