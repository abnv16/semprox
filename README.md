
# semantic-cache-proxy

An OpenAI-compatible Rust reverse proxy with local semantic embeddings
and an in-memory vector cache.

## Requirements

- Rust stable
- A CPU-compatible ONNX Runtime environment supported by FastEmbed
- Network access for upstream LLM requests
- Model artifacts available locally, or network access during initial setup

## Build

```bash
cargo build --release
```

## Run

```bash
export UPSTREAM_API_KEY="your-openai-api-key"

./target/release/semantic-cache-proxy \
  --port 8080 \
  --upstream-url https://api.openai.com \
  --similarity-threshold 0.90 \
  --max-cache-size 10000 \
  --model-path ./models
```

For fully offline model initialization, prepopulate the model cache
with the required artifacts before starting the service.

## First request: cache miss

```bash
curl -N http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [
      {"role": "user", "content": "Explain Kubernetes in one sentence"}
    ],
    "stream": true
  }'
```

The proxy forwards the request upstream and streams the result.
After the stream completes, it extracts the answer and attempts to cache it.

## Second request: cache hit

Repeat the request above, or send a semantically similar prompt:

```bash
curl http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [
      {"role": "user", "content": "Describe Kubernetes briefly"}
    ],
    "stream": false
  }'
```

If the embedding similarity exceeds the configured threshold,
the proxy returns the cached answer without contacting the upstream model.

Note: semantic similarity alone is not a correctness guarantee.
Use stricter admission rules for prompts requiring exact, personalized,
or safety-sensitive answers.

## Health and metrics

```bash
curl http://localhost:8080/health
curl http://localhost:8080/metrics
```

## Configuration

| Argument | Default | Description |
|---|---|---|
| `--port` | `8080` | Listen port |
| `--upstream-url` | `https://api.openai.com` | Upstream base URL |
| `--similarity-threshold` | `0.90` | Cosine similarity threshold |
| `--model-path` | Model cache default | Local model cache directory |
| `--max-cache-size` | `10000` | Maximum cache entry count |

Environment variables can be used for deployment configuration.
Use `UPSTREAM_API_KEY` for the upstream credential.

## Production considerations

- Add client authentication and tenant isolation.
- Restrict and validate upstream URLs.
- Implement a robust incremental SSE parser.
- Preserve full completion metadata and tool calls.
- Add rate limiting, admission control, and memory limits.
- Use model-aware cache keys and pricing-based cost estimates.
- Benchmark warm hits, misses, and concurrent streaming workloads.
