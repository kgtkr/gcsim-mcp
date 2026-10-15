use crate::config::Config;
use crate::limiter::SimLimiter;
use crate::mcp::help::GCSIM_HELP_TEXT;
use crate::sandbox::WasmSandbox;
use anyhow::Result;
use rmcp::{
    ErrorData as McpError, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct ReferenceCache {
    inner: Arc<RwLock<HashMap<String, String>>>,
}

impl ReferenceCache {
    pub async fn get(&self, category: &str) -> Option<String> {
        let cache = self.inner.read().await;
        cache.get(category).cloned()
    }

    pub async fn insert(&self, category: String, value: String) {
        let mut cache = self.inner.write().await;
        cache.insert(category, value);
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ValidateConfigArgs {
    #[schemars(description = "The complete GCSL configuration script to validate.")]
    pub config: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunSampleArgs {
    #[schemars(description = "The complete GCSL configuration script to execute.")]
    pub config: String,
    #[schemars(
        description = "Optional deterministic random seed (uint64). If omitted, a cryptographically secure random seed is used."
    )]
    pub seed: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetReferenceArgs {
    #[schemars(
        description = "Category of reference identifiers to look up. Must be 'characters', 'weapons', or 'artifacts'."
    )]
    pub category: String,
}

#[derive(Clone)]
pub struct GcsimMcpServer {
    pub sandbox: WasmSandbox,
    pub config: Arc<Config>,
    pub limiter: SimLimiter,
    pub ref_cache: ReferenceCache,
    pub tool_router: ToolRouter<Self>,
}

impl GcsimMcpServer {
    pub fn new(
        sandbox: WasmSandbox,
        config: Arc<Config>,
        limiter: SimLimiter,
        ref_cache: ReferenceCache,
    ) -> Self {
        Self {
            sandbox,
            config,
            limiter,
            ref_cache,
            tool_router: Self::tool_router(),
        }
    }
}

fn parse_wasm_output(output: String, fallback_err_prefix: &str) -> CallToolResult {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&output)
        && val.get("success").and_then(|s| s.as_bool()) == Some(false)
    {
        let err_msg = val
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("Unknown simulation error");
        return CallToolResult::error(vec![ContentBlock::text(format!(
            "{}: {}",
            fallback_err_prefix, err_msg
        ))]);
    }
    CallToolResult::success(vec![ContentBlock::text(output)])
}

#[tool_router]
impl GcsimMcpServer {
    #[tool(
        description = "Validate a Genshin Impact combat simulation configuration (GCSL). Checks syntax and returns error details or parsed team composition."
    )]
    async fn validate_gcsim_config(
        &self,
        Parameters(args): Parameters<ValidateConfigArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _permit = match self.limiter.try_acquire() {
            Ok(p) => p,
            Err(e) => {
                return Ok(CallToolResult::error(vec![ContentBlock::text(
                    e.to_string(),
                )]));
            }
        };

        let req_json = json!({
            "action": "validate",
            "config": args.config
        })
        .to_string();

        match self
            .sandbox
            .execute(&req_json, self.config.max_fuel_validate)
            .await
        {
            Ok(output) => Ok(parse_wasm_output(output, "Validation failed")),
            Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "Validation failed: {}",
                e
            ))])),
        }
    }

    #[tool(
        description = "Execute a single simulation sample in gcsim. Returns summary metrics including total DPS, character damage breakdown, action counts, and downtime due to cooldowns/energy."
    )]
    async fn run_gcsim_sample(
        &self,
        Parameters(args): Parameters<RunSampleArgs>,
    ) -> Result<CallToolResult, McpError> {
        let _permit = match self.limiter.try_acquire() {
            Ok(p) => p,
            Err(e) => {
                return Ok(CallToolResult::error(vec![ContentBlock::text(
                    e.to_string(),
                )]));
            }
        };

        let mut req_map = serde_json::Map::new();
        req_map.insert("action".into(), json!("sample"));
        req_map.insert("config".into(), json!(args.config));
        if let Some(s) = args.seed {
            req_map.insert("seed".into(), json!(s));
        }
        let req_json = serde_json::Value::Object(req_map).to_string();

        match self
            .sandbox
            .execute(&req_json, self.config.max_fuel_sample)
            .await
        {
            Ok(output) => Ok(parse_wasm_output(output, "Simulation failed")),
            Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "Simulation failed: {}",
                e
            ))])),
        }
    }

    #[tool(
        description = "Get valid gcsim identifiers and syntax rules for characters, weapons, or artifact sets. Use this to look up exact key names required in GCSL."
    )]
    async fn get_gcsim_reference(
        &self,
        Parameters(args): Parameters<GetReferenceArgs>,
    ) -> Result<CallToolResult, McpError> {
        let category = args.category;
        if category != "characters" && category != "weapons" && category != "artifacts" {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "Invalid category: '{}'. Must be one of: 'characters', 'weapons', 'artifacts'",
                category
            ))]));
        }

        if let Some(cached) = self.ref_cache.get(&category).await {
            return Ok(CallToolResult::success(vec![ContentBlock::text(cached)]));
        }

        let _permit = match self.limiter.try_acquire() {
            Ok(p) => p,
            Err(e) => {
                return Ok(CallToolResult::error(vec![ContentBlock::text(
                    e.to_string(),
                )]));
            }
        };

        let req_json = json!({
            "action": "reference",
            "category": category
        })
        .to_string();

        match self
            .sandbox
            .execute(&req_json, self.config.max_fuel_validate)
            .await
        {
            Ok(output) => {
                let res = parse_wasm_output(output.clone(), "Reference lookup failed");
                if res.is_error != Some(true) {
                    self.ref_cache.insert(category, output).await;
                }
                Ok(res)
            }
            Err(e) => Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "Failed to retrieve reference: {}",
                e
            ))])),
        }
    }

    #[tool(
        description = "Get general syntax help, template examples, and rules for writing gcsim configuration scripts (GCSL)."
    )]
    async fn get_gcsim_help(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            GCSIM_HELP_TEXT,
        )]))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for GcsimMcpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "gcsim combat simulation MCP server. Licensed under GNU AGPLv3. Source code: https://github.com/kgtkr/gcsim-mcp",
            )
    }
}

pub async fn run_stdio(server: GcsimMcpServer) -> Result<()> {
    tracing::info!("Starting gcsim MCP server in stdio mode");
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let handle = server.serve((stdin, stdout)).await?;
    handle.waiting().await?;
    Ok(())
}
