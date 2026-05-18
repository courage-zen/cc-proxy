# GitHub Actions Multi-Arch Docker Build Design

## Goal

Automate Rust binary compilation and Docker image building for AMD64 and ARM64 on every push to main, using GitHub Actions. Push resulting images to GHCR.

## Workflow Structure

Single workflow file: `.github/workflows/build.yml`

Trigger: push to `main` branch.

### Stage 1: Compile Rust Binaries (Matrix Strategy)

Two parallel jobs using GitHub Actions matrix:

| Matrix variable | Rust target | Docker arch |
|----------------|-------------|-------------|
| amd64 | `x86_64-unknown-linux-gnu` | `linux/amd64` |
| arm64 | `aarch64-unknown-linux-gnu` | `linux/arm64` |

Each job:
1. Checkout code
2. Install Rust toolchain (stable) + `cross`
3. `cross build --release --target <target>`
4. Upload compiled binary as GitHub Actions artifact

### Stage 2: Build and Push Docker Image

Depends on Stage 1 completion. Single job:
1. Download both artifacts (AMD64 + ARM64 binaries)
2. Place binaries into `docker/release/cc-proxy-amd64` and `docker/release/cc-proxy-arm64`
3. Set up `docker buildx` with QEMU for multi-arch build
4. Build multi-arch Docker manifest using existing `Dockerfile` (with `BASE_IMAGE` and `TARGETARCH` build args)
5. Push to GHCR

### Dockerfile Adaptation

The existing `Dockerfile` uses `ARG BASE_IMAGE` and `ARG TARGETARCH` to select the correct base image and binary per architecture. For `docker buildx` multi-arch builds, this works naturally — buildx invokes the Dockerfile once per platform in the manifest list, so `TARGETARCH` is automatically set to `amd64` or `arm64`.

However, the base image must also be multi-arch aware. Two approaches:

**Chosen approach: Separate base image names per arch (matches existing design).**

The existing build scripts already use separate base images: `multica-daemon-base:latest` (AMD64) and `multica-daemon-base-arm64:latest` (ARM64). In CI, we push these base images to GHCR as well, then reference them by architecture in the Dockerfile using a build arg.

The CI workflow will include a conditional mapping: when `TARGETARCH=amd64`, use `ghcr.io/<owner>/multica-daemon-base:latest`; when `TARGETARCH=arm64`, use `ghcr.io/<owner>/multica-daemon-base-arm64:latest`.

### Base Image Handling

The base images (`Dockerfile.base.claude` and `Dockerfile.base.claude.arm64`) contain Node.js, Python, Claude CLI, and git. These don't change frequently.

**Strategy:** Base images are built and pushed to GHCR manually (or via a separate workflow triggered on changes to `docker/Dockerfile.base.claude*`). The main build workflow references the pre-pushed base images from GHCR rather than rebuilding them every time.

This means we need a second, simpler workflow: `.github/workflows/build-base.yml`, triggered manually (`workflow_dispatch`) or on changes to base Dockerfiles.

## GHCR Authentication

- `GITHUB_TOKEN`: automatically provided by GitHub Actions, no configuration needed
- Package visibility: default is private; user can set to public after first push
- Login step: `docker login ghcr.io -u ${{ github.actor }} -p ${{ secrets.GITHUB_TOKEN }}`

## Image Tags

| Trigger | Tags |
|---------|------|
| push to main | `ghcr.io/<owner>/cc-proxy:latest`, `ghcr.io/<owner>/cc-proxy:<short SHA>` |

Base image tags:
- `ghcr.io/<owner>/multica-daemon-base:latest` (AMD64)
- `ghcr.io/<owner>/multica-daemon-base-arm64:latest` (ARM64)

## Files to Create

1. `.github/workflows/build.yml` — main build workflow (compile + Docker image)
2. `.github/workflows/build-base.yml` — base image build workflow (manual trigger)

## Security Considerations

- No secrets or credentials hardcoded in workflow files
- Uses `GITHUB_TOKEN` (auto-provided, scoped to the repository)
- Base images pushed to GHCR are private by default
- `config.yaml` with API keys is gitignored and never included in Docker context
