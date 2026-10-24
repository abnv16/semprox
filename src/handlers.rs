use crate::{cache::SemanticCache, embedder::Embedder};
use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use prometheus::{register_counter, register_int_gauge, Counter, Encoder, IntGauge, TextEncoder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Digest;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

pub static CACHE_HITS: Lazy<Counter> = Lazy::new(|| {
    register_counter!("semantic_cache_hits_total", "Total semantic cache hits")
        .expect("metric registration failed")
});

pub static CACHE_MISSES: Lazy<Counter> = Lazy::new(|| {
    register_counter!("semantic_cache_misses_total", "Total semantic cache misses")
        .expect("metric registration failed")
});

pub static REQUESTS_TOTAL: Lazy<Counter> = Lazy::new(|| {
    register_counter!(
        "semantic_cache_requests_total",
        "Total chat completion requests"
    )
    .expect("metric registration failed")
});

pub static CACHE_SIZE: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!("semantic_cache_entries_count", "Current cache entry count")
        .expect("metric registration failed")
});

pub struct AppState {
    pub cache: Arc<SemanticCache>,
    pub embedder: Arc<Mutex<Embedder>>,
    pub client: reqwest::Client,
    pub upstream_url: String,
    pub api_key: Option<String>,
    pub embedding_slots: Arc<Semaphore>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatCompletionRequest {
    pub model: String,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn extract_text_from_value(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    parts.push(text);
                } else if let Some(s) = item.as_str() {
                    parts.push(s);
                }
            }
            parts.join(" ")
        }
        _ => val.to_string(),
    }
}

fn extract_prompt(messages: &[ChatMessage]) -> String {
    for msg in messages.iter().rev() {
        if msg.role == "user" {
            let text = extract_text_from_value(&msg.content);
            if !text.trim().is_empty() {
                return text;
            }
        }
    }
    messages
        .iter()
        .map(|m| format!("{}: {}", m.role, extract_text_from_value(&m.content)))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn router(state: Arc<AppState>) -> axum::Router {
    // Force evaluation to register metrics with the default prometheus registry
    Lazy::force(&CACHE_HITS);
    Lazy::force(&CACHE_MISSES);
    Lazy::force(&REQUESTS_TOTAL);
    Lazy::force(&CACHE_SIZE);

    axum::Router::new()
        .route("/health", axum::routing::get(health_handler))
        .route("/metrics", axum::routing::get(metrics_handler))
        .route(
            "/v1/chat/completions",
            axum::routing::post(chat_completions_handler),
        )
        .with_state(state)
}

async fn health_handler() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

async fn metrics_handler() -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(_) => (
            StatusCode::OK,
            [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
            buffer,
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to encode metrics: {err}"),
        )
            .into_response(),
    }
}

fn cached_json_response(model: &str, content: &str, similarity: f32) -> Response {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut hasher = sha2::Sha256::new();
    hasher.update(content.as_bytes());
    let hash = hex::encode(&hasher.finalize()[..8]);
    let id = format!("chatcmpl-cached-{hash}");

    let resp_json = json!({
        "id": id,
        "object": "chat.completion",
        "created": created,
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 0,
            "completion_tokens": 0,
            "total_tokens": 0
        }
    });

    let mut response = Json(resp_json).into_response();
    response
        .headers_mut()
        .insert("X-Cache", HeaderValue::from_static("HIT"));
    if let Ok(sim_val) = HeaderValue::from_str(&format!("{:.4}", similarity)) {
        response.headers_mut().insert("X-Cache-Similarity", sim_val);
    }
    response
}

fn cached_stream_response(model: &str, content: &str, similarity: f32) -> Response {
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut hasher = sha2::Sha256::new();
    hasher.update(content.as_bytes());
    let hash = hex::encode(&hasher.finalize()[..8]);
    let id = format!("chatcmpl-cached-{hash}");

    let model_name = model.to_string();
    let content_str = content.to_string();

    let stream = async_stream::stream! {
        let chunk1 = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model_name,
            "choices": [{
                "index": 0,
                "delta": {
                    "role": "assistant",
                    "content": ""
                },
                "finish_reason": null
            }]
        });
        yield Ok::<_, std::io::Error>(Bytes::from(format!("data: {}\n\n", chunk1)));

        let chunk2 = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model_name,
            "choices": [{
                "index": 0,
                "delta": {
                    "content": content_str
                },
                "finish_reason": null
            }]
        });
        yield Ok(Bytes::from(format!("data: {}\n\n", chunk2)));

        let chunk3 = json!({
            "id": id,
            "object": "chat.completion.chunk",
            "created": created,
            "model": model_name,
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        yield Ok(Bytes::from(format!("data: {}\n\n", chunk3)));
        yield Ok(Bytes::from("data: [DONE]\n\n"));
    };

    let body = axum::body::Body::from_stream(stream);
    let mut response = Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("Connection", "keep-alive")
        .header("X-Cache", "HIT")
        .body(body)
        .unwrap_or_default();

    if let Ok(sim_val) = HeaderValue::from_str(&format!("{:.4}", similarity)) {
        response.headers_mut().insert("X-Cache-Similarity", sim_val);
    }
    response
}

async fn chat_completions_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    REQUESTS_TOTAL.inc();

    let req: ChatCompletionRequest = match serde_json::from_slice(&body) {
        Ok(parsed) => parsed,
        Err(err) => {
            warn!("failed to deserialize chat completion request: {err}");
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "message": format!("Invalid JSON request: {err}"),
                        "type": "invalid_request_error"
                    }
                })),
            )
                .into_response();
        }
    };

    let prompt = extract_prompt(&req.messages);

    // Compute embedding if prompt is present
    let embedding_opt = if !prompt.trim().is_empty() {
        let _permit = state.embedding_slots.acquire().await.ok();
        let mut embedder = state.embedder.lock();
        match embedder.embed(&prompt) {
            Ok(vec) => Some(vec),
            Err(e) => {
                warn!("failed to generate embedding: {e}");
                None
            }
        }
    } else {
        None
    };

    // Attempt cache lookup
    if let Some(ref embedding) = embedding_opt {
        if let Some(hit) = state.cache.lookup(&req.model, embedding) {
            CACHE_HITS.inc();
            info!(
                similarity = hit.similarity,
                matched_prompt = %hit.matched_prompt,
                "semantic cache hit"
            );

            if req.stream {
                return cached_stream_response(&req.model, &hit.content, hit.similarity);
            } else {
                return cached_json_response(&req.model, &hit.content, hit.similarity);
            }
        }
    }

    CACHE_MISSES.inc();
    info!(prompt = %prompt, "semantic cache miss - forwarding upstream");

    let upstream_endpoint = format!(
        "{}/v1/chat/completions",
        state.upstream_url.trim_end_matches('/')
    );

    let mut request_builder = state.client.post(&upstream_endpoint);

    // Set authorization header
    if let Some(ref api_key) = state.api_key {
        request_builder = request_builder.header("Authorization", format!("Bearer {api_key}"));
    } else if let Some(auth_val) = headers.get("Authorization") {
        request_builder = request_builder.header("Authorization", auth_val);
    }

    request_builder = request_builder
        .header("Content-Type", "application/json")
        .body(body.clone());

    let upstream_res = match request_builder.send().await {
        Ok(res) => res,
        Err(err) => {
            error!("upstream request failed: {err}");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": {
                        "message": format!("Upstream request failed: {err}"),
                        "type": "bad_gateway"
                    }
                })),
            )
                .into_response();
        }
    };

    let status = StatusCode::from_u16(upstream_res.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    if !status.is_success() {
        let err_bytes = upstream_res.bytes().await.unwrap_or_default();
        return (status, [("Content-Type", "application/json")], err_bytes).into_response();
    }

    if !req.stream {
        let resp_bytes = match upstream_res.bytes().await {
            Ok(b) => b,
            Err(e) => {
                error!("failed to read upstream response body: {e}");
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({ "error": { "message": "Failed to read upstream response" } })),
                )
                    .into_response();
            }
        };

        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&resp_bytes) {
            if let Some(content) = json_val
                .get("choices")
                .and_then(|c| c.get(0))
                .and_then(|c0| c0.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|cnt| cnt.as_str())
            {
                if let Some(embedding) = embedding_opt {
                    state
                        .cache
                        .insert(req.model, prompt, embedding, content.to_string());
                    CACHE_SIZE.set(state.cache.size() as i64);
                    info!(cache_size = state.cache.size(), "cached upstream response");
                }
            }
        }

        let mut response = (StatusCode::OK, resp_bytes).into_response();
        response
            .headers_mut()
            .insert("Content-Type", HeaderValue::from_static("application/json"));
        response
            .headers_mut()
            .insert("X-Cache", HeaderValue::from_static("MISS"));
        response
    } else {
        let mut upstream_stream = upstream_res.bytes_stream();
        let cache = state.cache.clone();
        let model = req.model.clone();

        let stream = async_stream::stream! {
            let mut line_buf = String::new();
            let mut full_content = String::new();

            while let Some(chunk_res) = upstream_stream.next().await {
                match chunk_res {
                    Ok(chunk) => {
                        let text = String::from_utf8_lossy(&chunk);
                        line_buf.push_str(&text);

                        while let Some(pos) = line_buf.find('\n') {
                            let line = line_buf[..pos].trim();
                            if let Some(data) = line.strip_prefix("data:") {
                                let payload = data.trim();
                                if payload != "[DONE]" && !payload.is_empty() {
                                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(payload) {
                                        if let Some(delta) = val
                                            .get("choices")
                                            .and_then(|c| c.get(0))
                                            .and_then(|c0| c0.get("delta"))
                                            .and_then(|d| d.get("content"))
                                            .and_then(|cnt| cnt.as_str())
                                        {
                                            full_content.push_str(delta);
                                        }
                                    }
                                }
                            }
                            line_buf.drain(..=pos);
                        }

                        yield Ok::<_, std::io::Error>(chunk);
                    }
                    Err(e) => {
                        error!("error streaming from upstream: {e}");
                        break;
                    }
                }
            }

            if !full_content.trim().is_empty() {
                if let Some(embedding) = embedding_opt {
                    cache.insert(model, prompt, embedding, full_content);
                    CACHE_SIZE.set(cache.size() as i64);
                    info!(cache_size = cache.size(), "cached streamed upstream response");
                }
            }
        };

        let body = axum::body::Body::from_stream(stream);
        Response::builder()
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .header("X-Cache", "MISS")
            .body(body)
            .unwrap_or_default()
    }
}
