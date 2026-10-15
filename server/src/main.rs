mod auth;
mod config;
mod limiter;
mod mcp;
mod sandbox;

use anyhow::Result;
use auth::auth_middleware;
use config::Config;
use limiter::SimLimiter;
use mcp::server::{GcsimMcpServer, ReferenceCache, run_stdio};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use sandbox::WasmSandbox;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "gcsim_mcp_server=info,tower_http=info".into()),
        )
        .init();

    let config = Config::load()?;

    info!(
        "Loaded configuration: wasm_path={}, max_fuel_validate={}, max_fuel_sample={}, max_memory={} bytes, max_concurrent={}",
        config.wasm_path,
        config.max_fuel_validate,
        config.max_fuel_sample,
        config.max_memory_bytes,
        config.max_concurrent_sims
    );

    let sandbox = WasmSandbox::new(&config.wasm_path, config.max_memory_bytes)?;
    let limiter = SimLimiter::new(config.max_concurrent_sims);
    let ref_cache = ReferenceCache::default();
    let config = Arc::new(config);

    let server = GcsimMcpServer::new(sandbox, config.clone(), limiter, ref_cache);

    if config.stdio {
        run_stdio(server).await?;
    } else {
        let auth_token = config.auth_token.clone();
        let host = config.host.clone();
        let port = config.port;

        let server_for_factory = server.clone();
        let service: StreamableHttpService<GcsimMcpServer, LocalSessionManager> =
            StreamableHttpService::new(
                move || Ok(server_for_factory.clone()),
                Default::default(),
                StreamableHttpServerConfig::default(),
            );

        let app = axum::Router::new()
            .nest_service("/mcp", service.clone())
            .fallback_service(service)
            .layer(axum::middleware::from_fn(move |req, next| {
                auth_middleware(auth_token.clone(), req, next)
            }))
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http());

        let addr = format!("{}:{}", host, port);
        info!(
            "Starting gcsim MCP Streamable HTTP server on http://{}",
            addr
        );

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
    }

    Ok(())
}
