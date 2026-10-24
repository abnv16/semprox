mod cache;
mod embedder;
mod handlers;

use anyhow::{bail, Result};
use cache::SemanticCache;
use clap::Parser;
use embedder::Embedder;
use handlers::{router, AppState};
use parking_lot::Mutex;
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "semprox",
    version,
    about = "OpenAI-compatible semantic caching reverse proxy"
)]
struct Args {
    #[arg(long, env = "PROXY_PORT", default_value_t = 8080)]
    port: u16,

    #[arg(long, env = "UPSTREAM_URL", default_value = "https://api.openai.com")]
    upstream_url: String,

    #[arg(long, env = "SIMILARITY_THRESHOLD", default_value_t = 0.90)]
    similarity_threshold: f32,

    #[arg(long, env = "MODEL_PATH")]
    model_path: Option<PathBuf>,

    #[arg(long, env = "MAX_CACHE_SIZE", default_value_t = 10_000)]
    max_cache_size: usize,

    #[arg(long, env = "UPSTREAM_API_KEY")]
    upstream_api_key: Option<String>,

    #[arg(long, env = "EMBEDDING_CONCURRENCY", default_value_t = 2)]
    embedding_concurrency: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    if !(0.0..=1.0).contains(&args.similarity_threshold) {
        bail!("similarity threshold must be between 0.0 and 1.0");
    }

    if args.max_cache_size == 0 {
        bail!("max cache size must be greater than zero");
    }

    if args.embedding_concurrency == 0 {
        bail!("embedding concurrency must be greater than zero");
    }

    let upstream_url = reqwest::Url::parse(&args.upstream_url)?;
    if !matches!(upstream_url.scheme(), "http" | "https") {
        bail!("upstream URL must use HTTP or HTTPS");
    }

    info!(
        port = args.port,
        upstream = %args.upstream_url,
        threshold = args.similarity_threshold,
        max_cache_size = args.max_cache_size,
        "starting semantic cache proxy"
    );

    // Model initialization can download model artifacts on first use.
    let embedder = Embedder::new(args.model_path)?;

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(300))
        .pool_max_idle_per_host(32)
        .build()?;

    let state = Arc::new(AppState {
        cache: Arc::new(SemanticCache::new(
            args.max_cache_size,
            args.similarity_threshold,
        )),
        embedder: Arc::new(Mutex::new(embedder)),
        client,
        upstream_url: args.upstream_url,
        api_key: args.upstream_api_key,
        embedding_slots: Arc::new(Semaphore::new(args.embedding_concurrency)),
    });

    let address = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = tokio::net::TcpListener::bind(address).await?;

    info!(address = %address, "HTTP server listening");

    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    info!("shutdown signal received");
}
