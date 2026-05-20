#!/usr/bin/env bash
# Full integration test: issue certs for two domains via Pebble, then test with Express.
# Prerequisites:
#   - docker compose up -d  (Pebble running)
#   - cargo build --release
#   - Domains added to /etc/hosts
#   - Challenge server port 5002 accessible from Pebble container

set -euo pipefail

BINARY="./target/release/acme-client"
ACME_URL="https://localhost:14000/dir"
OUTPUT="./certs"

echo "=== Fetching Pebble CA ==="
bash scripts/fetch-pebble-ca.sh

echo ""
echo "=== Issuing cert for test1.example.com ==="
"$BINARY" issue \
  --acme-url "$ACME_URL" \
  --domain test1.example.com \
  --email admin@example.com \
  --output "$OUTPUT" \
  --insecure \
  --challenge-bind "0.0.0.0:5002"

echo ""
echo "=== Issuing cert for test2.example.com ==="
"$BINARY" issue \
  --acme-url "$ACME_URL" \
  --domain test2.example.com \
  --domain www.test2.example.com \
  --email admin@example.com \
  --output "$OUTPUT" \
  --insecure \
  --challenge-bind "0.0.0.0:5002"

echo ""
echo "=== Showing stored certificates ==="
"$BINARY" show --domain test1.example.com --output "$OUTPUT"
"$BINARY" show --domain test2.example.com --output "$OUTPUT"

echo ""
echo "=== Starting Express HTTPS app ==="
cd test-app && npm install --silent
DOMAIN=test1.example.com PORT=8443 CERTS_DIR=../certs node server.js &
APP_PID=$!
sleep 2

echo ""
echo "=== Testing with curl ==="
# --ssl-no-revoke: needed on Windows (Schannel doesn't check OCSP for Pebble certs)
curl --cacert ../docker/pebble-root-ca.pem --ssl-no-revoke https://test1.example.com:8443/ | jq .
curl --cacert ../docker/pebble-root-ca.pem --ssl-no-revoke https://test1.example.com:8443/health | jq .

kill $APP_PID 2>/dev/null || true
echo ""
echo "All tests passed."
