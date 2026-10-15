use anyhow::{Result, anyhow};
use clap::Parser;

/// 数量文字列（Fuelやメモリサイズ等）を u64 にパースする共通パーサー
/// - IEC 2進接頭辞 (1024の冪): ki/Ki (*1,024), mi/Mi (*1,048,576), gi/Gi (*1,073,741,824)
/// - SI 10進接頭辞 (1000の冪): k/K (*1,000), m/M (*1,000,000), g/G (*1,000,000,000)
/// - サフィックスなし: 数値そのまま
pub fn parse_scaled_u64(input: &str) -> Result<u64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("input string is empty"));
    }

    let (num_str, multiplier) = if trimmed.ends_with("ki") || trimmed.ends_with("Ki") {
        (&trimmed[..trimmed.len() - 2], 1024u64)
    } else if trimmed.ends_with("mi") || trimmed.ends_with("Mi") {
        (&trimmed[..trimmed.len() - 2], 1024u64 * 1024)
    } else if trimmed.ends_with("gi") || trimmed.ends_with("Gi") {
        (&trimmed[..trimmed.len() - 2], 1024u64 * 1024 * 1024)
    } else if trimmed.ends_with('k') || trimmed.ends_with('K') {
        (&trimmed[..trimmed.len() - 1], 1_000u64)
    } else if trimmed.ends_with('m') || trimmed.ends_with('M') {
        (&trimmed[..trimmed.len() - 1], 1_000_000u64)
    } else if trimmed.ends_with('g') || trimmed.ends_with('G') {
        (&trimmed[..trimmed.len() - 1], 1_000_000_000u64)
    } else {
        (trimmed, 1u64)
    };

    let base: u64 = num_str
        .trim()
        .parse()
        .map_err(|e| anyhow!("invalid number '{}': {}", num_str, e))?;

    base.checked_mul(multiplier)
        .ok_or_else(|| anyhow!("value overflow"))
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "gcsim MCP Server")]
pub struct RawConfig {
    /// 起動モード (stdio または http)
    #[arg(long, default_value_t = false)]
    pub stdio: bool,

    /// HTTP/SSE バインドホスト
    #[arg(long, env = "GCSIM_HOST", default_value = "0.0.0.0")]
    pub host: String,

    /// HTTP/SSE バインドポート
    #[arg(long, env = "GCSIM_PORT", default_value_t = 3485)]
    pub port: u16,

    /// WASIバイナリのパス
    #[arg(long, env = "WASM_PATH", default_value = "./gcsim.wasm")]
    pub wasm_path: String,

    /// オプショナル認証トークン (未設定時は認証なし)
    #[arg(long, env = "AUTH_TOKEN")]
    pub auth_token: Option<String>,

    /// validate実行時の最大Fuel (例: "50M", "50000000")
    #[arg(long, env = "MAX_FUEL_VALIDATE", default_value = "50M")]
    pub max_fuel_validate: String,

    /// sample実行時の最大Fuel (例: "500M", "500000000")
    #[arg(long, env = "MAX_FUEL_SAMPLE", default_value = "500M")]
    pub max_fuel_sample: String,

    /// Wasm最大メモリ上限 (例: "256Mi", "512Mi")
    #[arg(long, env = "MAX_MEMORY", default_value = "256Mi")]
    pub max_memory: String,

    /// 同時シミュレーション実行数上限 (Semaphore)
    #[arg(long, env = "MAX_CONCURRENT_SIMS", default_value_t = 4)]
    pub max_concurrent_sims: usize,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub stdio: bool,
    pub host: String,
    pub port: u16,
    pub wasm_path: String,
    pub auth_token: Option<String>,
    pub max_fuel_validate: u64,
    pub max_fuel_sample: u64,
    pub max_memory_bytes: usize,
    pub max_concurrent_sims: usize,
}

impl Config {
    pub fn load() -> Result<Self> {
        let raw = RawConfig::parse();
        Ok(Self {
            stdio: raw.stdio,
            host: raw.host,
            port: raw.port,
            wasm_path: raw.wasm_path,
            auth_token: raw.auth_token,
            max_fuel_validate: parse_scaled_u64(&raw.max_fuel_validate)?,
            max_fuel_sample: parse_scaled_u64(&raw.max_fuel_sample)?,
            max_memory_bytes: parse_scaled_u64(&raw.max_memory)? as usize,
            max_concurrent_sims: raw.max_concurrent_sims,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_scaled_u64() {
        assert_eq!(parse_scaled_u64("100").unwrap(), 100);
        assert_eq!(parse_scaled_u64("50k").unwrap(), 50_000);
        assert_eq!(parse_scaled_u64("50K").unwrap(), 50_000);
        assert_eq!(parse_scaled_u64("50m").unwrap(), 50_000_000);
        assert_eq!(parse_scaled_u64("50M").unwrap(), 50_000_000);
        assert_eq!(parse_scaled_u64("1g").unwrap(), 1_000_000_000);
        assert_eq!(parse_scaled_u64("1G").unwrap(), 1_000_000_000);

        assert_eq!(parse_scaled_u64("1ki").unwrap(), 1024);
        assert_eq!(parse_scaled_u64("1Ki").unwrap(), 1024);
        assert_eq!(parse_scaled_u64("256mi").unwrap(), 256 * 1024 * 1024);
        assert_eq!(parse_scaled_u64("256Mi").unwrap(), 256 * 1024 * 1024);
        assert_eq!(parse_scaled_u64("2gi").unwrap(), 2 * 1024 * 1024 * 1024);
        assert_eq!(parse_scaled_u64("2Gi").unwrap(), 2 * 1024 * 1024 * 1024);
    }
}
