<div align="center">

# ⚡ semantic-cache-proxy

**A high-performance, OpenAI-compatible semantic caching reverse proxy written in Rust.**

[![CI](https://github.com/abhinavp/semantic-cache-proxy/actions/workflows/ci.yml/badge.svg)](https://github.com/abhinavp/semantic-cache-proxy/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![Docker](https://img.shields.io/badge/docker-supported-2496ED.svg)](Dockerfile)

Slash your LLM API costs and reduce response latencies from seconds to milliseconds using local ONNX embeddings and an in-memory vector cache.

[Features](#-key-features) • [Architecture](#-architecture) • [Quickstart](#-quickstart) • [Usage](#-usage-examples) • [Configuration](#-configuration) • [Contributing](#-contributing)

</div>

---

## 💡 Why Semantic Cache Proxy?

Traditional exact-match caching (like Redis key-value hashing) fails when users ask the same question in slightly different words:

- `"Explain Kubernetes in one sentence"`
- `"Describe Kubernetes briefly"`
- `"What is Kubernetes in simple terms?"`

Standard caches treat these as separate requests, hitting upstream LLMs every time, incurring monetary token costs and 1,000ms–5,000ms latency.

**`semantic-cache-proxy`** sits transparently between your application and OpenAI (or any OpenAI-compatible API). It computes semantic embeddings locally using CPU-optimized ONNX models (FastEmbed) and evaluates cosine similarity against previous responses. If a match exceeds your similarity threshold (e.g., `0.90`), it returns the cached completion instantly—**in less than 5ms**.

---

## 🚀 Key Features

- **🎯 Zero External Dependencies**: No Redis, Pinecone, Qdrant, or external vector DB required. Runs as a single, lightweight binary.
- **⚡ Ultra-Low Latency**: In-memory normalized vector dot-product matching achieves sub-millisecond similarity scans.
- **🔄 Drop-in OpenAI Compatibility**: Supports `/v1/chat/completions` seamlessly. Works with LangChain, LlamaIndex, OpenAI SDKs, and raw HTTP clients.
- **🌊 Bidirectional Streaming Support**: Handles both Server-Sent Events (`stream: true`) and standard JSON responses (`stream: false`). Automatically parses streaming deltas to populate cache entries.
- **🧠 Local ONNX Embeddings**: Powered by `fastembed-rs` (`AllMiniLML6V2Q` / `bge-small-en-v1.5`), executing entirely on CPU without GPU overhead.
- **🧹 Thread-Safe LRU Eviction**: Bounded memory footprint with automatic least-recently-used eviction when `max_cache_size` is reached.
- **📊 Production Observability**: Built-in Prometheus metrics at `/metrics` and health readiness checks at `/health`.

---

## 🏛 Architecture

```mermaid
flowchart TD
    Client(["Application / Client"])
    Proxy["semantic-cache-proxy"]
    FastEmbed["Local FastEmbed (ONNX)"]
    VectorStore[("In-Memory Vector Cache<br/>(Cosine Similarity + LRU)")]
    Upstream["Upstream LLM<br/>(OpenAI / vLLM / Ollama)"]

    Client -->|POST /v1/chat/completions| Proxy
    Proxy -->|1. Generate Embedding| FastEmbed
    FastEmbed -->|Vector [384 dims]| Proxy
    Proxy -->|2. Cosine Similarity Scan| VectorStore

    VectorStore -->|Sim >= Threshold: HIT| Proxy
    Proxy -->|3a. Return Cached Response (< 5ms)| Client

    VectorStore -->|Sim < Threshold: MISS| Proxy
    Proxy -->|3b. Forward Request| Upstream
    Upstream -->|Stream / JSON Response| Proxy
    Proxy -->|4. Return Response| Client
    Proxy -.->|5. Async Cache Answer| VectorStore
```

---

## 🏁 Quickstart

### Option 1: Docker Compose (Recommended)

1. Clone the repository:
   ```bash
   git clone https://github.com/abhinavp/semantic-cache-proxy.git
   cd semantic-cache-proxy
   ```
2. Start the proxy:
   ```bash
   docker compose up -d
   ```
3. Test health:
   ```bash
   curl http://localhost:8080/health
   # {"status":"ok"}
   ```

### Option 2: Build and Run from Source

#### Prerequisites
- Rust stable (1.80+)
- Standard C/C++ compiler for ONNX runtime linkage

```bash
# Build optimized release binary
cargo build --release

# Run proxy with custom threshold
RUST_LOG=info ./target/release/semantic-cache-proxy \
  --port 8080 \
  --upstream-url https://api.openai.com \
  --similarity-threshold 0.90 \
  --max-cache-size 10000
```

---

## 📖 Usage Examples

Set your OpenAI API key (or specify via `--upstream-api-key`):

```bash
export OPENAI_API_KEY="sk-..."
```

### 1. First Request: Cache Miss (Forwarded Upstream)

Send a streaming chat completion request:

```bash
curl -N http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [
      {"role": "user", "content": "Explain Kubernetes in one sentence"}
    ],
    "stream": true
  }'
```

*Header received: `X-Cache: MISS`*
The proxy streams the upstream response to you in real-time, extracts the answer tokens, and caches the embedding.

---

### 2. Second Request: Semantically Similar (Cache Hit)

Now send a semantically similar prompt:

```bash
curl -i http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [
      {"role": "user", "content": "Describe Kubernetes briefly in one sentence"}
    ],
    "stream": false
  }'
```

*Response Headers:*
```http
HTTP/1.1 200 OK
content-type: application/json
x-cache: HIT
x-cache-similarity: 0.9412
```

The response is returned **immediately without hitting OpenAI**!

---

### 3. Monitoring & Metrics

`semantic-cache-proxy` exports standard Prometheus metrics on `GET /metrics`:

```bash
curl http://localhost:8080/metrics
```

```text
# HELP semantic_cache_hits_total Total semantic cache hits
# TYPE semantic_cache_hits_total counter
semantic_cache_hits_total 1

# HELP semantic_cache_misses_total Total semantic cache misses
# TYPE semantic_cache_misses_total counter
semantic_cache_misses_total 1

# HELP semantic_cache_requests_total Total chat completion requests
# TYPE semantic_cache_requests_total counter
semantic_cache_requests_total 2

# HELP semantic_cache_entries_count Current cache entry count
# TYPE semantic_cache_entries_count gauge
semantic_cache_entries_count 1
```

---

## ⚙ Configuration

All settings can be configured via CLI arguments or environment variables:

| Argument | Environment Variable | Default | Description |
|---|---|---|---|
| `--port` | `PROXY_PORT` | `8080` | Port for the HTTP server to listen on |
| `--upstream-url` | `UPSTREAM_URL` | `https://api.openai.com` | Base URL of upstream LLM API |
| `--upstream-api-key` | `UPSTREAM_API_KEY` | *(None)* | Optional default API key for upstream (overrides client header) |
| `--similarity-threshold` | `SIMILARITY_THRESHOLD` | `0.90` | Cosine similarity threshold (`0.0` – `1.0`) for cache admission |
| `--max-cache-size` | `MAX_CACHE_SIZE` | `10000` | Maximum number of items in cache before LRU eviction |
| `--embedding-concurrency`| `EMBEDDING_CONCURRENCY`| `2` | Max concurrent embedding jobs |
| `--model-path` | `MODEL_PATH` | *(Default cache)* | Directory path for local model weights |

See [.env.example](.env.example) for a pre-configured template.

---

## 🛠 Development & Testing

Run unit tests:
```bash
cargo test
```

Check code formatting and run Clippy:
```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

---

## 🤝 Contributing

Contributions are welcome! Please review our [Contributing Guide](CONTRIBUTING.md) and [Code of Conduct](CODE_OF_CONDUCT.md) before opening a pull request.
