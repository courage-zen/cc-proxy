<div align="center">

# CC Proxy

### All-in-One HTTP Proxy for Claude Code, Codex, Gemini CLI, OpenCode, OpenClaw & Hermes Agent

[![Rust](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

---

## What is CC Proxy?

CC Proxy is a **pure Rust CLI tool** that runs a local HTTP proxy server, routing AI coding tool requests (Claude Code, Codex, Gemini CLI, etc.) through a configurable provider system with automatic failover.

Instead of manually editing config files, you manage everything from the command line.

**Core capabilities:**
- Local HTTP proxy on `127.0.0.1:15721`
- Format conversion between Claude / Codex / Gemini API formats
- Circuit breaker — pauses failing providers after 5 consecutive errors
- Auto-failover — routes to the next healthy provider automatically
- Per-app proxy routing (Claude-only, Codex-only, Gemini-only, or all together)
- Config backup before any modification

---

## Origin

CC Proxy is a fork of [cc-switch](https://github.com/farion1231/cc-switch) by **[@farion1231](https://github.com/farion1231)** — a Tauri-based GUI + CLI hybrid tool. Special thanks to farion1231 for creating and maintaining the original project.

This fork strips away the GUI (Tauri, WebView, frontend assets), keeping only the pure Rust CLI. The following cleanup was done:

- Removed Tauri, webview, and all frontend dependencies from `Cargo.toml`
- Removed `src-tauri/` directory (GUI code)
- Deleted unused modules: `session_manager`, `auto_launch`, `claude_plugin`, `auto_sync`, `linux_fix`
- Renamed all `cc_switch` / `cc-switch` identifiers to `cc_proxy` / `cc-proxy`
- Replaced hardcoded Google OAuth credentials with environment variables (`GEMINI_OAUTH_CLIENT_ID`, `GEMINI_OAUTH_CLIENT_SECRET`)
- Kept the full proxy server, provider management, health monitoring, CLI, and config layer

---

## Quick Start

```bash
# Build from source
git clone https://github.com/courage-zen/cc-proxy.git
cd cc-proxy
cargo build --release
./target/release/cc-proxy start

# Or install via Homebrew (macOS/Linux)
brew tap farion1231/ccproxy
brew install cc-proxy
```

## CLI Commands

```bash
# Start proxy (foreground)
cc-proxy start

# Config management
cc-proxy config list          # list all config keys
cc-proxy config get <key>     # read a config value
cc-proxy config set <key> <value>  # write a config value

# Provider management
cc-proxy provider list        # list all providers
cc-proxy provider models <name>  # show available models
cc-proxy provider health      # health check all providers
cc-proxy provider test-endpoint <url>  # ping an endpoint
cc-proxy provider test-model --name <provider> [--model <model>]

# Failover
cc-proxy failover switch <name>   # manually switch to a provider
```

## Configuration

Default config directory: `~/.config/cc-proxy/`

```yaml
# proxy.yaml
proxy:
  listen: "127.0.0.1"
  port: 15721
  apps:
    claude: true
    codex: false
    gemini: false
  health_check:
    timeout: 10
    interval: 60
  failover:
    circuit_breaker_threshold: 5
    recovery_timeout: 30

logging:
  level: "info"   # debug / info / warn / error
```

Environment variables for OAuth (required for Gemini):
```bash
export GEMINI_OAUTH_CLIENT_ID="your-client-id"
export GEMINI_OAUTH_CLIENT_SECRET="your-client-secret"
```

## Data Storage

- **Config**: `~/.config/cc-proxy/proxy.yaml`
- **Database**: `~/.cc-proxy/cc-proxy.db` (SQLite — providers, MCP, prompts, skills)
- **Settings**: `~/.cc-proxy/settings.json`
- **Backups**: `~/.cc-proxy/backups/`

Override paths via environment variables:
- `CC_SWITCH_CONFIG_DIR` — config directory
- `CC_SWITCH_TEST_HOME` — home directory (for testing isolation)

## Architecture

```
cc-proxy CLI
  ├── Commands (clap)
  ├── Services (provider, failover, health, subscription)
  ├── Config (yaml, json, toml parsers per tool)
  └── Proxy Server (axum + tokio)
       ├── HTTP router
       ├── Format converter (Claude ↔ Codex ↔ Gemini)
       └── Circuit breaker + auto-failover
```

## Development

```bash
cargo build --release
cargo check
cargo fmt
cargo clippy
cargo test
```

## License

MIT — see [LICENSE](LICENSE)