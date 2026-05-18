#!/bin/bash
set -e

# Write Claude CLI settings to route through cc-proxy
mkdir -p ~/.claude
cat > ~/.claude/settings.json <<'SETTINGS'
{
  "apiBaseUrl": "http://127.0.0.1:15721"
}
SETTINGS

# Start cc-proxy in foreground (PID 1)
exec cc-proxy start -c /etc/cc-proxy
