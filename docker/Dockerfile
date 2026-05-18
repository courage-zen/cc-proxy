# Final application image: cc-proxy + Claude CLI on top of base image
# Pre-built cc-proxy binary must be placed in release/ directory before building.
#
# Build for x86_64:
#   ./build.sh amd64
# Build for arm64:
#   ./build.sh arm64

ARG BASE_IMAGE=multica-daemon-base:latest
FROM ${BASE_IMAGE}

ARG TARGETARCH=amd64

# Copy pre-built cc-proxy binary for the target architecture
COPY release/cc-proxy-${TARGETARCH} /usr/local/bin/cc-proxy
RUN chmod +x /usr/local/bin/cc-proxy

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Default config directory (user mounts config.yaml here)
ENV CC_SWITCH_CONFIG_DIR=/etc/cc-proxy

ENTRYPOINT ["/entrypoint.sh"]
