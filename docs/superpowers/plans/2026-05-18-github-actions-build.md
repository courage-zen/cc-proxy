# GitHub Actions Multi-Arch Build Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Set up GitHub Actions workflows to compile cc-proxy Rust binary for AMD64 and ARM64, build multi-arch Docker images, and push to GHCR.

**Architecture:** Two workflows — `build-base.yml` for manually building and pushing base images to GHCR, and `build.yml` for the main pipeline that cross-compiles Rust binaries (matrix strategy) and builds multi-arch Docker images via `docker buildx`. The existing `Dockerfile` already supports `ARG BASE_IMAGE` and `ARG TARGETARCH`, enabling buildx to select the correct base image and binary per platform.

**Tech Stack:** GitHub Actions, `cross` (Rust cross-compilation), `docker buildx`, QEMU, GHCR

---

## File Structure

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `.github/workflows/build.yml` | Main CI: compile binaries + build Docker image, triggered on push to main |
| Create | `.github/workflows/build-base.yml` | Build and push base images to GHCR, triggered manually |

---

### Task 1: Create base image build workflow

**Files:**
- Create: `.github/workflows/build-base.yml`

This workflow builds the two base images (AMD64 and ARM64) and pushes them to GHCR. It is triggered manually via `workflow_dispatch` so base images are only rebuilt when needed (their contents — Node.js, Claude CLI, Python — change infrequently).

- [ ] **Step 1: Create `.github/workflows/` directory and `build-base.yml`**

```yaml
name: Build Base Images

on:
  workflow_dispatch:

env:
  REGISTRY: ghcr.io
  IMAGE_NAME_AMD64: ${{ github.repository }}/multica-daemon-base
  IMAGE_NAME_ARM64: ${{ github.repository }}/multica-daemon-base-arm64

jobs:
  build-base-amd64:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Log in to GHCR
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME_AMD64 }}
          tags: |
            type=raw,value=latest

      - name: Build and push AMD64 base image
        uses: docker/build-push-action@v6
        with:
          context: ./docker
          file: ./docker/Dockerfile.base.claude
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          platforms: linux/amd64

  build-base-arm64:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3

      - name: Log in to GHCR
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME_ARM64 }}
          tags: |
            type=raw,value=latest

      - name: Build and push ARM64 base image
        uses: docker/build-push-action@v6
        with:
          context: ./docker
          file: ./docker/Dockerfile.base.claude.arm64
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          platforms: linux/arm64
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/build-base.yml
git commit -m "ci: add base image build workflow for GHCR"
```

---

### Task 2: Create main build workflow

**Files:**
- Create: `.github/workflows/build.yml`

This is the core CI workflow. It has two stages:
1. **compile** job (matrix): Cross-compiles the Rust binary for AMD64 and ARM64 in parallel, uploads the binary as an artifact.
2. **docker** job: Downloads both artifacts, builds multi-arch Docker image with `docker buildx`, pushes to GHCR.

The `Dockerfile` uses `ARG BASE_IMAGE` and `ARG TARGETARCH`. In the docker job, we pass `BASE_IMAGE` conditionally based on `TARGETARCH` using a shell script that builds per-platform and creates a multi-arch manifest.

- [ ] **Step 1: Create `build.yml`**

```yaml
name: Build and Push Docker Image

on:
  push:
    branches: [main]

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}/cc-proxy

jobs:
  compile:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        include:
          - arch: amd64
            target: x86_64-unknown-linux-gnu
          - arch: arm64
            target: aarch64-unknown-linux-gnu
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable

      - name: Install cross
        run: cargo install cross --locked

      - name: Cache Cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ matrix.target }}-${{ hashFiles('**/Cargo.lock') }}
          restore-keys: |
            ${{ runner.os }}-cargo-${{ matrix.target }}-

      - name: Build with cross
        run: cross build --release --target ${{ matrix.target }}

      - name: Upload binary artifact
        uses: actions/upload-artifact@v4
        with:
          name: cc-proxy-${{ matrix.arch }}
          path: target/${{ matrix.target }}/release/cc-proxy
          retention-days: 1

  docker:
    runs-on: ubuntu-latest
    needs: compile
    permissions:
      contents: read
      packages: write
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Download AMD64 binary
        uses: actions/download-artifact@v4
        with:
          name: cc-proxy-amd64
          path: docker/release/cc-proxy-amd64

      - name: Download ARM64 binary
        uses: actions/download-artifact@v4
        with:
          name: cc-proxy-arm64
          path: docker/release/cc-proxy-arm64

      - name: Make binaries executable
        run: |
          chmod +x docker/release/cc-proxy-amd64
          chmod +x docker/release/cc-proxy-arm64

      - name: Set up QEMU
        uses: docker/setup-qemu-action@v3

      - name: Set up Docker Buildx
        uses: docker/setup-buildx-action@v3

      - name: Log in to GHCR
        uses: docker/login-action@v3
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}

      - name: Extract metadata
        id: meta
        uses: docker/metadata-action@v5
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          tags: |
            type=raw,value=latest
            type=sha,prefix=

      - name: Build and push multi-arch Docker image
        uses: docker/build-push-action@v6
        with:
          context: ./docker
          file: ./docker/Dockerfile
          push: true
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          platforms: linux/amd64,linux/arm64
          build-args: |
            BASE_IMAGE_AMD64=${{ env.REGISTRY }}/${{ github.repository }}/multica-daemon-base:latest
            BASE_IMAGE_ARM64=${{ env.REGISTRY }}/${{ github.repository }}/multica-daemon-base-arm64:latest
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/build.yml
git commit -m "ci: add main build workflow for multi-arch Docker image"
```

---

### Task 3: Update Dockerfile to support multi-arch buildx with conditional base image

**Files:**
- Modify: `docker/Dockerfile`

The current `Dockerfile` uses a single `ARG BASE_IMAGE` which doesn't work with `docker buildx` multi-platform builds — buildx runs the Dockerfile once per platform, but we need it to pick a different base image per architecture.

The solution: use Docker's automatic `TARGETARCH` variable (set by buildx) to conditionally select the base image via a multi-stage Dockerfile pattern. We add two `ARG`s for base images (`BASE_IMAGE_AMD64`, `BASE_IMAGE_ARM64`) and a build stage that picks the right one based on `TARGETARCH`.

- [ ] **Step 1: Update `docker/Dockerfile`**

Replace the entire content with:

```dockerfile
# Final application image: cc-proxy + Claude CLI on top of base image
# Pre-built cc-proxy binary must be placed in release/ directory before building.
#
# Multi-arch build (via GitHub Actions):
#   docker buildx build --platform linux/amd64,linux/arm64 ...
#
# Single-arch local build:
#   ./build.sh amd64
#   ./build.sh arm64

# Build args for base images per architecture (used by CI buildx)
ARG BASE_IMAGE_AMD64=multica-daemon-base:latest
ARG BASE_IMAGE_ARM64=multica-daemon-base-arm64:latest

# Select the correct base image based on TARGETARCH (auto-set by buildx)
FROM ${BASE_IMAGE_AMD64} AS base-amd64
FROM ${BASE_IMAGE_ARM64} AS base-arm64

# Conditional stage: picks the right base for the target platform
ARG TARGETARCH=amd64
FROM base-${TARGETARCH}

# Copy pre-built cc-proxy binary for the target architecture
COPY release/cc-proxy-${TARGETARCH} /usr/local/bin/cc-proxy
RUN chmod +x /usr/local/bin/cc-proxy

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Default config directory (user mounts config.yaml here)
ENV CC_SWITCH_CONFIG_DIR=/etc/cc-proxy

ENTRYPOINT ["/entrypoint.sh"]
```

- [ ] **Step 2: Verify local build still works**

Confirm the `docker/build.sh` script still works with the updated Dockerfile. The existing `build.sh` passes `--build-arg BASE_IMAGE=...` but the new Dockerfile uses `BASE_IMAGE_AMD64` / `BASE_IMAGE_ARM64` instead, so we need to update `build.sh` as well.

Run: `cat docker/build.sh` — review whether it needs changes (it does, see next task).

- [ ] **Step 3: Commit**

```bash
git add docker/Dockerfile
git commit -m "feat: update Dockerfile for multi-arch buildx support"
```

---

### Task 4: Update build.sh to work with new Dockerfile

**Files:**
- Modify: `docker/build.sh`

The existing `build.sh` passes `--build-arg BASE_IMAGE=...` which no longer matches the new `BASE_IMAGE_AMD64` / `BASE_IMAGE_ARM64` args. Update it to pass the correct build arg per architecture.

- [ ] **Step 1: Update `docker/build.sh`**

Replace lines 44-53 (the `docker build` command) with:

```bash
echo "==> Building ${FINAL_IMAGE}:${IMAGE_TAG} (${ARCH})..."
echo "    Base image: ${BASE_IMAGE}"
echo "    Binary: ${BINARY}"

if [[ "${ARCH}" == "arm64" ]]; then
    BASE_ARG="BASE_IMAGE_ARM64=${BASE_IMAGE}"
else
    BASE_ARG="BASE_IMAGE_AMD64=${BASE_IMAGE}"
fi

docker build \
  -f Dockerfile \
  --build-arg "${BASE_ARG}" \
  --build-arg "TARGETARCH=${ARCH}" \
  -t "${FINAL_IMAGE}:${IMAGE_TAG}" \
  .
```

- [ ] **Step 2: Commit**

```bash
git add docker/build.sh
git commit -m "fix: update build.sh to pass per-arch base image arg"
```

---

### Task 5: First run — build and push base images to GHCR

This is a manual step to run after the workflow files are merged to main. The main build workflow depends on base images existing in GHCR.

- [ ] **Step 1: Push all changes to GitHub**

```bash
git push origin main
```

- [ ] **Step 2: Trigger base image build workflow**

Go to GitHub repo → Actions → "Build Base Images" → "Run workflow" → Click "Run workflow".

This will:
- Build `Dockerfile.base.claude` for AMD64 and push to `ghcr.io/farion1231/cc-proxy/multica-daemon-base:latest`
- Build `Dockerfile.base.claude.arm64` for ARM64 (via QEMU) and push to `ghcr.io/farion1231/cc-proxy/multica-daemon-base-arm64:latest`

- [ ] **Step 3: Wait for base image workflow to complete**

Check the Actions tab — both jobs (`build-base-amd64` and `build-base-arm64`) should succeed.

- [ ] **Step 4: Set GHCR packages to public (optional)**

Go to GitHub → Packages → each package → Package settings → Danger Zone → Change visibility → Public.

This is needed if you want to pull images without authentication.

- [ ] **Step 5: Verify main build workflow triggers**

The next push to main will trigger the `build.yml` workflow automatically. Verify it compiles both architectures and pushes the multi-arch Docker image to GHCR.
