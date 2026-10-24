# Contributing to semantic-cache-proxy

Thank you for your interest in contributing to **semantic-cache-proxy**! We welcome bug reports, feature requests, documentation improvements, and code contributions from everyone.

This document outlines the workflow and best practices to ensure an easy and productive contribution experience.

---

## Code of Conduct

All contributors and participants are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md). Please read it to understand the community standards we expect.

---

## Getting Started

### Prerequisites

- **Rust**: Stable toolchain (Rust 1.80+ recommended). Install via [rustup](https://rustup.rs/):
  ```bash
  rustup update stable
  rustup component add clippy rustfmt
  ```
- **C/C++ Build Tools**: A standard C/C++ compiler and build tools (e.g., `build-essential` on Debian/Ubuntu or Xcode Command Line Tools on macOS) for ONNX Runtime integration via FastEmbed.
- **Git**

### Clone & Build

1. Fork the repository on GitHub and clone your fork:
   ```bash
   git clone https://github.com/<your-username>/semantic-cache-proxy.git
   cd semantic-cache-proxy
   ```
2. Build the project:
   ```bash
   cargo build
   ```
3. Run tests to ensure everything is working:
   ```bash
   cargo test
   ```

---

## Development Workflow

### Running Locally

To run the proxy locally with info-level logging:

```bash
RUST_LOG=info cargo run -- --port 8080 --similarity-threshold 0.90
```

> **Note**: On the first start, FastEmbed will download the default quantized embedding model (`bge-small-en-v1.5` / `all-MiniLM-L6-v2`) into `.fastembed_cache/`. Subsequent runs load the model instantly from disk.

### Testing Endpoints

Once the server is running, you can test health and metrics in another terminal:

```bash
# Health check
curl http://localhost:8080/health

# Prometheus metrics
curl http://localhost:8080/metrics

# Test completions proxying
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [
      {"role": "user", "content": "Explain Kubernetes in one sentence"}
    ],
    "stream": false
  }'
```

---

## Code Quality & Verification

Before submitting a Pull Request, please ensure your changes pass all local verification steps:

1. **Format Check**:
   ```bash
   cargo fmt --all -- --check
   ```
   To auto-format code, run:
   ```bash
   cargo fmt --all
   ```

2. **Linting (Clippy)**:
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```

3. **Automated Tests**:
   ```bash
   cargo test --all
   ```

---

## Codebase Architecture

Familiarize yourself with the core modules:

- **`src/main.rs`**: Entry point, command-line arguments parsing via `clap`, application state initialization, and TCP listener setup.
- **`src/embedder.rs`**: FastEmbed wrapper managing ONNX Runtime model lifecycle and vector generation.
- **`src/cache.rs`**: Thread-safe in-memory vector cache with cosine similarity computation, threshold-based matching, and LRU eviction.
- **`src/handlers.rs`**: Axum web handlers for `/health`, `/metrics`, and `/v1/chat/completions` (supporting bidirectional streaming SSE and JSON proxying).

---

## Pull Request Guidelines

1. **Branch Naming**: Use descriptive branch names:
   - `feat/add-redis-backend`
   - `fix/sse-delta-parsing`
   - `docs/update-architecture`
2. **Commit Messages**: Follow standard conventional commits format:
   - `feat: add support for dynamic similarity thresholds`
   - `fix: resolve race condition during LRU cache eviction`
   - `docs: update deployment guidelines in README`
3. **Tests**: Add unit or integration tests for any new features or bug fixes.
4. **Documentation**: Update the `README.md` or code comments if your change alters configuration or public behavior.

---

## Reporting Issues

- **Bug Reports**: Use our [Bug Report Template](.github/ISSUE_TEMPLATE/bug_report.md). Include your OS, Rust version, reproduction steps, and relevant logs.
- **Feature Requests**: Use our [Feature Request Template](.github/ISSUE_TEMPLATE/feature_request.md). Describe the use case and proposed solution.
