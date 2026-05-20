#!/usr/bin/env bash
# Add test domains to /etc/hosts (requires sudo / admin privileges on Windows).
# On Windows run this from an elevated Git Bash or WSL shell.

HOSTS_FILE="/etc/hosts"
DOMAINS=(
  "test1.example.com"
  "test2.example.com"
  "www.test1.example.com"
)

for domain in "${DOMAINS[@]}"; do
  if grep -q "$domain" "$HOSTS_FILE"; then
    echo "Already present: $domain"
  else
    echo "127.0.0.1  $domain" | sudo tee -a "$HOSTS_FILE"
    echo "Added: $domain"
  fi
done

echo "Done."
