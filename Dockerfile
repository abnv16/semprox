# Stage 1: Build
FROM rust:1.81-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy dependency manifests and lockfile
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY README.md ./

# Build release binary
RUN cargo build --release

# Stage 2: Minimal runtime image
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# Install runtime dependencies required by FastEmbed / ONNX Runtime and HTTPS
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libstdc++6 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy compiled binary from builder stage
COPY --from=builder /app/target/release/semprox /usr/local/bin/semprox

# Create directory for model cache
RUN mkdir -p /app/models && chown -R 1000:1000 /app

USER 1000:1000

ENV PROXY_PORT=8080 \
    UPSTREAM_URL=https://api.openai.com \
    SIMILARITY_THRESHOLD=0.90 \
    MAX_CACHE_SIZE=10000 \
    MODEL_PATH=/app/models \
    RUST_LOG=info

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
  CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["semprox"]
