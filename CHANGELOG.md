# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-10-24

### Added
- **OpenAI-Compatible Reverse Proxy**: Drop-in proxy support for `/v1/chat/completions` handling standard JSON requests and bidirectional Server-Sent Events (SSE) streaming (`stream: true`).
- **Local Semantic Embeddings**: Built-in ONNX Runtime embedding engine using FastEmbed (`all-MiniLM-L6-v2` / `bge-small-en-v1.5`), executing 100% on CPU without GPU overhead or external vector databases.
- **In-Memory Vector Cache**: Normalized dot-product cosine similarity search with configurable similarity thresholds (`--similarity-threshold`) and automated LRU eviction (`--max-cache-size`).
- **Streaming Response Parser**: Incremental SSE stream reader that accumulates completion chunks and caches responses asynchronously once the stream finishes.
- **Prometheus Observability**: Native `/metrics` endpoint exporting cache hit/miss counts, total requests, and active cache size metrics.
- **Health Check**: `/health` endpoint for Kubernetes and container readiness probes.
- **Containerization**: Multi-stage `Dockerfile` and `docker-compose.yml` for zero-configuration container deployment.
- **CI / Developer Tooling**: GitHub Actions workflow (`ci.yml`) enforcing `cargo test`, `cargo fmt`, and `cargo clippy -D warnings`.
