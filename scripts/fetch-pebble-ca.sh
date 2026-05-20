#!/usr/bin/env bash
# Fetch the Pebble root CA certificate so curl can verify our issued certs.
# Run after: docker compose up -d

set -euo pipefail

OUTPUT="${1:-docker/pebble-root-ca.pem}"

echo "Fetching Pebble root CA..."
curl -sk https://localhost:15000/roots/0 > "$OUTPUT"
echo "Saved to $OUTPUT"

# Also fetch intermediate CA
curl -sk https://localhost:15000/intermediates/0 >> "$OUTPUT"
echo "Appended intermediate CA to $OUTPUT"
