# gcsim MCP サーバー 実装計画書 (Rust + Go WASI)

本書は、原神の戦闘シミュレータ「gcsim」をモデルコンテキストプロトコル（MCP: Model Context Protocol）サーバー化し、ローカル環境（stdio）およびWeb公開環境（HTTP/SSE）の双方で安全かつ高パフォーマンスに運用できる独立プロジェクト（`gcsim-mcp`）の実装計画書です。

---

## 1. プロジェクト概要 & 設計方針

1. **目的**:
   - LLM（Claude, Cursor, 各種エージェント）が自律的にgcsimの戦闘設定（GCSL: Genshin Combat Script Language）を検証（`validate`）および実行（`sample`）し、DPS計算やローテーション最適化を自動で行える環境を提供する。
2. **決定論的サンドボックス制御（決定性 & 再現性 & 安全性）**:
   - 実行計算制限は **Wasmtimeの Fuel（WebAssembly命令実行数カウンター）のみ** で制御する。
   - ホストマシンの性能や瞬間的な負荷、OSスケジューラに左右される非決定的な実時間タイムアウト（Epoch Interruption等）は一切使用しない。
   - **Fuel制限の必須化 & 単位表記（K/M/G/Ki/Mi/Gi）対応**:
     - 無限ループ（`while(1){}`）や過大な計算によるDoSを100%確実に遮断するため、**Fuel制限は必須（デフォルト値あり）** とする。
     - デフォルト値:
       - `MAX_FUEL_VALIDATE`: `50M` (50,000,000 命令)
       - `MAX_FUEL_SAMPLE`: `500M` (500,000,000 命令)
3. **厳格なメモリ上限制限（StoreLimits & ResourceLimiter）**:
   - 悪意あるスクリプトやバグによる過大なメモリ割り当て（ヒープ枯渇攻撃 / OOM）からホストシステムを保護するため、Wasmtimeの `StoreLimits` による線形メモリ最大サイズ制限を適用する。
   - 環境変数 `MAX_MEMORY` で設定可能（デフォルト: `256Mi` = 268,435,456 バイト）。
   - 上限到達時はWasm内で安全にメモリアロケーションが失敗（GoランタイムのOOMパニックを補足）し、ホストプロセスを巻き込まずにエラーを返却する。
4. **統一された共通単位パーサー**:
   - 命令数（Fuel）とメモリサイズ（Byte）で同一の共通パーサーを利用する。
   - **SI接頭辞（1000の冪乗）**: `K` (*1,000), `M` (*1,000,000), `G` (*1,000,000,000)
   - **IEC 2進接頭辞（1024の冪乗）**: `Ki` (*1,024), `Mi` (*1,048,576), `Gi` (*1,073,741,824)
   - サフィックスなし（数値のみ）はそのまま解釈。
5. **再現可能なビルド & デプロイ（Nix `dockerTools`）**:
   - Dockerfileは使用せず、**Nix Flakes および `pkgs.dockerTools`** を採用する。
   - Go WASI モジュール、Rust MCP サーバー、最小構成のコンテナイメージ（OCI/Docker）をすべて Nix により100%再現可能（Reproducible）にビルド・パッケージングする。
6. **オプショナルな認証**:
   - 環境変数 `AUTH_TOKEN` が設定されている場合、HTTP/SSE エンドポイントへのアクセスに対して Bearer Token 認証を要求する。
   - `AUTH_TOKEN` が未設定の場合は **認証なし** で誰でも利用可能なパブリックモードとして動作する。
7. **独立性 & 依存関係の最小化**:
   - gcsim本体のリポジトリ（`github.com/genshinsim/gcsim`）をフォーク・改変せず、Goモジュールとして直接依存関係に取り込む。
8. **LLM最適化レスポンス**:
   - gcsimの生ログ（数MB〜数十MBに達する全フレームイベントログ）はそのまま返さず、Goモジュール内で `agg.Summarize` を用いて集約し、総DPS・キャラ別ダメージ割合・スキル稼働時間・エネルギーやCD待機によるロス時間等に要約したクリーンなJSON（数KB）を返却する。

---

## 2. リポジトリ構成

```text
gcsim-mcp/
├── wasm-engine/              # Go製 WASI モジュール
│   ├── go.mod                # github.com/genshinsim/gcsim を import
│   ├── go.sum
│   └── main.go               # stdin/stdout で JSON を受発注し、シミュレーション・要約を行う
├── server/                   # Rust製 MCP サーバー
│   ├── Cargo.toml
│   ├── Cargo.lock
│   └── src/
│       ├── main.rs           # エントリーポイント (stdio / HTTP+SSE 切替)
│       ├── config.rs         # 環境変数・CLIオプション設定管理 (共通単位パーサー含む)
│       ├── auth.rs           # オプショナルなBearer Token認証ミドルウェア
│       ├── sandbox.rs        # Wasmtime ホスト (Fuel制御, StoreLimitsメモリ制限, モジュール管理)
│       ├── mcp/
│       │   ├── server.rs     # rmcp による GcsimMcpServer (Streamable HTTP & stdio) & ReferenceCache
│       │   └── help.rs       # GCSL 構文ヘルプ定数テキスト
│       └── limiter.rs        # 同時実行制限 (Semaphore)
├── flake.nix                 # Nix Flake (Go WASI, Rust, dockerTools ビルド定義)
├── flake.lock
├── plan.md                   # 本計画書
└── README.md
```

---

## 3. 実装詳細

### Phase 1: Go WASI モジュール (`wasm-engine/`)

gcsimのコアパッケージ（`pkg/gcs/ast`, `pkg/gcs/parser`, `pkg/simulation`, `pkg/agg`）を呼び出し、標準入出力経由でコマンドを処理するWASI Preview 1バイナリを作成します。

#### ① `wasm-engine/go.mod`

```go
module gcsim-mcp/wasm-engine

go 1.22

require github.com/genshinsim/gcsim v0.0.0-20240901000000-xxxxxxxxxxxx // 最新コミット/タグ
```

#### ② `wasm-engine/main.go`

- **責務**:
  - `stdin` から JSON リクエストを読み取る。
  - `action: "validate"`: 構文チェックを行い、有効性、エラー詳細、およびパースされたパーティ構成キャラ一覧等のメタデータを返す。
  - `action: "sample"`: 指定シード（または乱数）で1イテレーションを実行。`agg.Summarize` によりLLMが解釈しやすい要約データ（DPS、キャラ別ダメージ、失敗待機時間等）に変換して返す。
  - `stdout` に JSON レスポンスを出力して終了する。

- **コード実装骨子**:

```go
package main

import (
	"encoding/json"
	"fmt"
	"io"
	"os"

	"github.com/genshinsim/gcsim/pkg/agg"
	"github.com/genshinsim/gcsim/pkg/gcs/ast"
	"github.com/genshinsim/gcsim/pkg/gcs/eval"
	"github.com/genshinsim/gcsim/pkg/gcs/parser"
	"github.com/genshinsim/gcsim/pkg/simulation"
	"github.com/genshinsim/gcsim/pkg/simulator"
)

type Request struct {
	Action string `json:"action"` // "validate" | "sample"
	Config string `json:"config"`
	Seed   uint64 `json:"seed,omitempty"`
}

type Response struct {
	Success bool            `json:"success"`
	Data    json.RawMessage `json:"data,omitempty"`
	Error   string          `json:"error,omitempty"`
}

type ValidateResult struct {
	Valid      bool     `json:"valid"`
	Characters []string `json:"characters,omitempty"`
	Duration   float64  `json:"duration_seconds,omitempty"`
	Targets    int      `json:"target_count,omitempty"`
}

type CharacterSummary struct {
	Name             string             `json:"name"`
	TotalDamage      float64            `json:"total_damage"`
	DamagePercentage float64            `json:"damage_percentage"`
	DPS              float64            `json:"dps"`
	ActiveTimeSec    float64            `json:"active_time_seconds"`
	Actions          map[string]float64 `json:"actions"`
	Reactions        map[string]float64 `json:"reactions"`
	DamageBySource   map[string]float64 `json:"damage_by_source"`
	Failures         agg.FailureTimes   `json:"failures"`
}

type SampleSummary struct {
	Seed            uint64             `json:"seed"`
	DurationSec     float64            `json:"duration_seconds"`
	TotalDamage     float64            `json:"total_damage"`
	DPS             float64            `json:"dps"`
	Characters      []CharacterSummary `json:"characters"`
	OverallFailures agg.FailureTimes   `json:"overall_failures"`
}

func main() {
	var resp Response
	defer func() {
		if r := recover(); r != nil {
			resp = Response{Success: false, Error: fmt.Sprintf("simulation engine panic: %v", r)}
		}
		_ = json.NewEncoder(os.Stdout).Encode(resp)
	}()

	input, err := io.ReadAll(os.Stdin)
	if err != nil {
		resp = Response{Success: false, Error: "failed to read stdin: " + err.Error()}
		return
	}

	var req Request
	if err := json.Unmarshal(input, &req); err != nil {
		resp = Response{Success: false, Error: "invalid request JSON: " + err.Error()}
		return
	}

	switch req.Action {
	case "validate":
		file := ast.NewFile()
		p := parser.New(file, req.Config)
		simcfg, _, err := p.Parse()
		if err != nil {
			resp = Response{Success: false, Error: err.Error()}
			return
		}
		if len(simcfg.Errors) > 0 {
			errMsgs := ""
			for _, v := range simcfg.Errors {
				errMsgs += fmt.Sprintf("%v\n", v)
			}
			resp = Response{Success: false, Error: errMsgs}
			return
		}

		chars := make([]string, len(simcfg.Characters))
		for i, c := range simcfg.Characters {
			chars[i] = c.Base.Key.String()
		}

		valData := ValidateResult{
			Valid:      true,
			Characters: chars,
			Duration:   simcfg.Settings.Duration,
			Targets:    len(simcfg.Targets),
		}
		bytes, _ := json.Marshal(valData)
		resp = Response{Success: true, Data: bytes}

	case "sample":
		seed := req.Seed
		if seed == 0 {
			seed = uint64(simulator.CryptoRandSeed())
		}

		file := ast.NewFile()
		p := parser.New(file, req.Config)
		simcfg, gcsl, err := p.Parse()
		if err != nil {
			resp = Response{Success: false, Error: "parse error: " + err.Error()}
			return
		}

		cpycfg := simcfg.Copy()
		program := gcsl.Copy()

		core, err := simulation.NewCore(int64(seed), false, cpycfg)
		if err != nil {
			resp = Response{Success: false, Error: "core init error: " + err.Error()}
			return
		}

		evaluator, err := eval.NewEvaluator(file, program, core)
		if err != nil {
			resp = Response{Success: false, Error: "evaluator error: " + err.Error()}
			return
		}

		sim, err := simulation.New(cpycfg, evaluator, core)
		if err != nil {
			resp = Response{Success: false, Error: "simulation init error: " + err.Error()}
			return
		}

		result, err := sim.Run()
		if err != nil {
			resp = Response{Success: false, Error: "simulation run error: " + err.Error()}
			return
		}

		// agg.Summarize で軽量要約を作成
		summary := agg.Summarize(&result)
		durationSec := float64(summary.Duration) / 60.0

		charSummaries := make([]CharacterSummary, len(summary.Characters))
		for i := range summary.Characters {
			cs := &summary.Characters[i]
			name := simcfg.Characters[i].Base.Key.String()

			actions := make(map[string]float64)
			for k, v := range cs.Actions.All() {
				actions[k] = v
			}
			reactions := make(map[string]float64)
			for k, v := range cs.Reactions.All() {
				reactions[k] = v
			}
			bySource := make(map[string]float64)
			for k, v := range cs.DamageBySource.All() {
				bySource[k] = v
			}

			charDPS := 0.0
			dmgPct := 0.0
			if durationSec > 0 {
				charDPS = cs.Damage / durationSec
			}
			if summary.TotalDamage > 0 {
				dmgPct = (cs.Damage / summary.TotalDamage) * 100.0
			}

			charSummaries[i] = CharacterSummary{
				Name:             name,
				TotalDamage:      cs.Damage,
				DamagePercentage: dmgPct,
				DPS:              charDPS,
				ActiveTimeSec:    float64(cs.ActiveTime) / 60.0,
				Actions:          actions,
				Reactions:        reactions,
				DamageBySource:   bySource,
				Failures:         cs.Failures,
			}
		}

		sampleData := SampleSummary{
			Seed:            seed,
			DurationSec:     durationSec,
			TotalDamage:     summary.TotalDamage,
			DPS:             summary.DPS,
			Characters:      charSummaries,
			OverallFailures: summary.Failures,
		}

		bytes, _ := json.Marshal(sampleData)
		resp = Response{Success: true, Data: bytes}

	default:
		resp = Response{Success: false, Error: "unknown action: " + req.Action}
	}
}
```

---

### Phase 2: Rust 設定・認証・サンドボックス層 (`server/src/`)

#### ① `server/Cargo.toml`

```toml
[package]
name = "gcsim-mcp-server"
version = "0.1.0"
edition = "2021"

[dependencies]
wasmtime = { version = "25.0", features = ["async"] }
wasmtime-wasi = "25.0"
tokio = { version = "1", features = ["full"] }
axum = { version = "0.7", features = ["macros", "tokio"] }
axum-extra = { version = "0.9", features = ["typed-header"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
clap = { version = "4.4", features = ["derive", "env"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
anyhow = "1.0"
uuid = { version = "1.7", features = ["v4"] }
async-stream = "0.3"
tokio-stream = "0.1"
```

#### ② 設定管理 & 共通単位パーサー (`server/src/config.rs`)

命令数（Fuel）とメモリサイズ（Byte）で同一の共通パーサーを利用します。

```rust
use anyhow::{anyhow, Result};
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

    let base: u64 = num_str.trim().parse()
        .map_err(|e| anyhow!("invalid number '{}': {}", num_str, e))?;

    base.checked_mul(multiplier)
        .ok_or_else(|| anyhow!("value overflow"))
}

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "gcsim MCP Server")]
pub struct RawConfig {
    /// 起動モード (stdio または http)
    #[arg(long, default_value = "false")]
    pub stdio: bool,

    /// HTTP/SSE バインドホスト
    #[arg(long, env = "GCSIM_HOST", default_value = "0.0.0.0")]
    pub host: String,

    /// HTTP/SSE バインドポート
    #[arg(long, env = "GCSIM_PORT", default_value_t = 8080)]
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
```

#### ③ オプショナル認証ミドルウェア (`server/src/auth.rs`)

`config.auth_token` が `Some` の場合のみ、HTTPリクエストヘッダー `Authorization: Bearer <token>` を検証します。`None` の場合は全リクエストを通過させます。

```rust
use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};

pub async fn auth_middleware(
    expected_token: Option<String>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Some(token) = expected_token {
        let auth_header = req
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        match auth_header {
            Some(header_val) if header_val.starts_with("Bearer ") => {
                let provided = &header_val["Bearer ".len()..];
                if provided != token {
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
            _ => return Err(StatusCode::UNAUTHORIZED),
        }
    }

    Ok(next.run(req).await)
}
```

#### ④ 決定論的サンドボックス層 (`server/src/sandbox.rs`)

- **決定論的 Fuel 制御 & メモリ上限（StoreLimits）**:
  - `config.consume_fuel(true)` を設定。
  - `HostState` に `WasiP1Ctx` と `StoreLimits` を同居させ、`ResourceLimiter` トレイトを実装。
  - `StoreLimitsBuilder::new().memory_size(max_memory_bytes).build()` を `Store` に設定し、超過したアロケーションは Wasm 内で安全に拒絶。
  - Fuel超過時は `Trap::OutOfFuel` を捕捉し、LLMに明確に「計算命令上限 (Fuel: X) に達しました」と返答。
  - **非決定的な時間制限（Epoch interruption等）は完全排除**。
- **ライフサイクル & メモリ管理**:
  - サーバー起動時に `wasmtime::Engine` と `wasmtime::Module` をプリロード（メモリ保持）。
  - リクエストごとに軽量な `Store` と `Instance` を作成。
  - 入出力は `wasmtime_wasi::pipe::MemoryInputPipe` と `MemoryOutputPipe` を使用し、ホストファイルシステムやネットワーク権限は一切付与しない。

```rust
use anyhow::{anyhow, Result};
use std::sync::Arc;
use wasmtime::*;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::pipe::{MemoryInputPipe, MemoryOutputPipe};

pub struct HostState {
    pub wasi: WasiP1Ctx,
    pub limits: StoreLimits,
}

impl ResourceLimiter for HostState {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> Result<bool, wasmtime::Error> {
        self.limits.memory_growing(current, desired, maximum)
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> Result<bool, wasmtime::Error> {
        self.limits.table_growing(current, desired, maximum)
    }
}

#[derive(Clone)]
pub struct WasmSandbox {
    engine: Engine,
    module: Module,
    linker: Arc<Linker<HostState>>,
    max_memory_bytes: usize,
}

impl WasmSandbox {
    pub fn new(wasm_path: &str, max_memory_bytes: usize) -> Result<Self> {
        let mut config = Config::new();
        config.async_support(true);
        config.consume_fuel(true); // 決定論的命令カウンターを有効化

        let engine = Engine::new(&config)?;
        let module = Module::from_file(&engine, wasm_path)?;

        let mut linker = Linker::new(&engine);
        preview1::add_to_linker_async(&mut linker, |state: &mut HostState| &mut state.wasi)?;

        Ok(Self {
            engine,
            module,
            linker: Arc::new(linker),
            max_memory_bytes,
        })
    }

    pub async fn execute(&self, input_json: &str, max_fuel: u64) -> Result<String> {
        let stdin = MemoryInputPipe::new(input_json.as_bytes().to_vec());
        let stdout = MemoryOutputPipe::new(10 * 1024 * 1024); // 最大10MB出力バッファ

        let wasi_ctx = wasmtime_wasi::WasiCtxBuilder::new()
            .stdin(stdin)
            .stdout(stdout.clone())
            .build_p1();

        let limits = StoreLimitsBuilder::new()
            .memory_size(self.max_memory_bytes)
            .instances(1)
            .tables(10)
            .build();

        let host_state = HostState {
            wasi: wasi_ctx,
            limits,
        };

        let mut store = Store::new(&self.engine, host_state);
        store.limiter(|state| state);
        store.set_fuel(max_fuel)?;

        let instance = self.linker.instantiate_async(&mut store, &self.module).await?;
        let start_fn = instance.get_typed_func::<(), ()>(&mut store, "_start")?;

        let res = start_fn.call_async(&mut store, ()).await;

        if let Err(e) = res {
            if let Some(trap) = e.downcast_ref::<Trap>() {
                if *trap == Trap::OutOfFuel {
                    return Err(anyhow!("Instruction fuel limit exceeded (max_fuel={}). Potential infinite loop or overly large simulation.", max_fuel));
                }
            }
            return Err(e);
        }

        let output_bytes = stdout.contents();
        let output_str = String::from_utf8(output_bytes.into())?;
        Ok(output_str)
    }
}
```

---

### Phase 3: MCP サーバー & ツール実装 (`server/src/mcp/`)

#### ① 提供する MCP ツール仕様 (`tools.rs`)

| ツール名                | 説明                                                                                     | 主要引数                                                |
| :---------------------- | :--------------------------------------------------------------------------------------- | :------------------------------------------------------ |
| `validate_gcsim_config` | GCSL設定の構文チェックを行い、有効性、エラー行、キャラ編成を返す                         | `config` (string, required)                             |
| `run_gcsim_sample`      | 1イテレーションを実行し、総DPS・キャラ別ダメージ割合・アクション稼働状況・ロス時間を返す | `config` (string, required), `seed` (integer, optional) |
| `get_gcsim_help`        | LLMがGCSLを正しく記述するための構文リファレンスとテンプレートプロンプトを返す            | なし                                                    |

- **MCP Tool Input Schema (JSON)**:

```json
{
  "name": "run_gcsim_sample",
  "description": "Execute a single simulation sample of Genshin Impact combat in gcsim. Returns summary metrics including total DPS, character damage breakdown, action counts, and downtime due to energy/cooldown issues.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "config": {
        "type": "string",
        "description": "The complete GCSL configuration script."
      },
      "seed": {
        "type": "integer",
        "description": "Optional deterministic random seed (uint64). If omitted, a cryptographically secure random seed is generated."
      }
    },
    "required": ["config"]
  }
}
```

- **`get_gcsim_help` が提供するプロンプト内容**:
  - GCSLの基本構造: `options`, `character`, `target`, `active`, アクションループ文法。
  - 必須ルール: `options iteration=1 duration=90;` 等。
  - キャラクター定義構文: `char_name char lvl=90/90 cons=0 talent=9,9,9;`、武器・聖遺物ステータスの指定法。
  - 頻出エラー対策: 交代クールダウン（1秒）、エネルギー不足時のスキル先行待機設定など。

#### ② トランスポート実装 (`server/src/mcp/protocol.rs`)

1. **stdio モード**:
   - `--stdio` 引数で起動。
   - 標準入力から1行ごとに改行区切り JSON-RPC 2.0 リクエストを読み取り、標準出力にレスポンスを1行で出力。
   - Claude Desktop や Cursor の `mcpServers` 設定に直接指定して利用。
2. **HTTP / SSE モード (Web公開用)**:
   - 2024-11-05 MCP仕様準拠。
   - `GET /sse`:
     - クライアントが接続すると、UUIDで `session_id` を発行。
     - SSEイベント `endpoint` で `/messages?sessionId=<uuid>` をクライアントに通知。
     - セッション管理用チャンネル（`tokio::sync::mpsc`）を開いて保持。
   - `POST /messages?sessionId=<uuid>`:
     - JSON-RPC 2.0 リクエストを受信。
     - 即時 202 Accepted を返し、Wasm実行結果は該当セッションのSSEストリーム経由で非同期送信。
   - `auth_token` が有効な場合、両エンドポイントでBearer認証をチェック。

---

### Phase 4: 同時実行制御層 (`server/src/limiter.rs`)

サーバーリソース（CPU/メモリ）の枯渇を防ぐための並列度制御：

- **グローバル同時実行セマフォ (`limiter.rs`)**:
  - `tokio::sync::Semaphore::new(max_concurrent_sims)` でWasmシミュレーション実行の並列度を制御。
  - スロットが空いていない場合はブロッキング待機せず、即座に MCPエラー（または HTTP 429: Server busy）を返却。

---

## 4. 環境変数一覧

| 環境変数名            | デフォルト値         | 説明                                                                   |
| :-------------------- | :------------------- | :--------------------------------------------------------------------- |
| `AUTH_TOKEN`          | _未設定（認証なし）_ | 設定時、HTTP/SSE へのアクセスに `Authorization: Bearer <token>` を要求 |
| `MAX_FUEL_VALIDATE`   | `50M`                | validate 実行時の最大WebAssembly命令数（`50M`, `50000000` 等）         |
| `MAX_FUEL_SAMPLE`     | `500M`               | sample 実行時の最大WebAssembly命令数（`500M`, `500000000` 等）         |
| `MAX_MEMORY`          | `256Mi`              | Wasmインスタンスの最大線形メモリ上限（`256Mi`, `512Mi`, `256M` 等）    |
| `GCSIM_HOST`          | `0.0.0.0`            | HTTP/SSE サーバーのバインドホスト                                      |
| `GCSIM_PORT`          | `8080`               | HTTP/SSE サーバーのバインドポート                                      |
| `WASM_PATH`           | `./gcsim.wasm`       | コンパイル済み WASI モジュールの配置パス                               |
| `MAX_CONCURRENT_SIMS` | `4`                  | Wasmシミュレーションの最大同時実行スロット数                           |

---

## 5. テスト・検証シナリオ

実装完了後、以下のテストシナリオを実行して動作および安全性を検証します：

1. **基本機能検証**:
   - 代表的なパーティ（例: 雷電ナショナル、スクロース武装）の設定で `validate_gcsim_config` を実行し、`valid: true` とキャラ名一覧が返ること。
   - 同設定で `run_gcsim_sample` を実行し、妥当なDPS（例: 30,000〜60,000）およびキャラ別ダメージ割合の合計が約100%になること。
   - `get_gcsim_help` を実行し、構文テンプレートが取得できること。
2. **決定論的命令上限テスト（無限ループ遮断）**:
   - 無限ループ（`while(1){}`）を含むGCSL設定を `run_gcsim_sample` に送信。
   - **期待結果**: サーバーがフリーズせず、Fuel上限（デフォルト500M）到達により即座に `OutOfFuel` エラーが返ること。
3. **メモリ上限テスト（OOM防止）**:
   - `MAX_MEMORY=64Mi` などの極小値で起動し、大規模な設定を与えた場合にホストプロセスがクラッシュ（OOM Killer）せず、安全にWasmエラーとして補足・返却されること。
4. **共通単位パーステスト**:
   - `MAX_FUEL_SAMPLE=100M`（SI: 100,000,000）や `MAX_MEMORY=512Mi`（IEC: 536,870,912）で起動し、単一のパーサーで正しく解釈されること。
5. **オプショナル認証テスト**:
   - `AUTH_TOKEN=secret123` を設定し、トークンなしリクエストが 401 Unauthorized になること。
   - 正しいBearerトークンを付与した場合に正常通信できること。
   - `AUTH_TOKEN` 未設定時にトークンなしで正常通信できること。
6. **同時実行制限テスト**:
   - `MAX_CONCURRENT_SIMS=1` の状態で並列リクエストを送り、溢れたリクエストが即時拒絶（Server busy）されること。

---

## 6. Nix によるパッケージング & dockerTools によるコンテナ構築 (`flake.nix`)

Dockerfileは使用せず、**Nix Flakes** と **`pkgs.dockerTools.buildLayeredImage`** を利用して、Wasmバイナリ、Rustサーバー、最小限のコンテナイメージを完全に再現可能（Reproducible）にビルドします。

### `flake.nix` 実装仕様

```nix
{
  description = "gcsim MCP Server (Rust + Go WASI)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # 1. Go WASI モジュール (gcsim.wasm) のビルド
        gcsimWasm = pkgs.buildGoModule {
          pname = "gcsim-wasm";
          version = "0.1.0";
          src = ./wasm-engine;

          # go.sum から vendorHash を指定
          vendorHash = pkgs.lib.fakeHash; # 初期ビルド時に正しいハッシュに置換

          CGO_ENABLED = 0;
          GOOS = "wasip1";
          GOARCH = "wasm";
          ldflags = [ "-s" "-w" ];

          # 出力パス: $out/share/gcsim.wasm
          installPhase = ''
            runHook preInstall
            mkdir -p $out/share
            install -m 644 $GOPATH/bin/wasip1_wasm/wasm-engine $out/share/gcsim.wasm || \
            install -m 644 $GOPATH/bin/wasm-engine $out/share/gcsim.wasm
            runHook postInstall
          '';
        };

        # 2. Rust製 MCP サーバーバイナリのビルド
        gcsimMcpServer = pkgs.rustPlatform.buildRustPackage {
          pname = "gcsim-mcp-server";
          version = "0.1.0";
          src = ./server;

          cargoLock = {
            lockFile = ./server/Cargo.lock;
          };
        };

        # 3. pkgs.dockerTools によるコンテナイメージ
        dockerImage = pkgs.dockerTools.buildLayeredImage {
          name = "gcsim-mcp";
          tag = "latest";

          contents = [
            pkgs.cacert # SSL/TLS 証明書
          ];

          config = {
            Cmd = [ "${gcsimMcpServer}/bin/gcsim-mcp-server" ];
            Env = [
              "WASM_PATH=${gcsimWasm}/share/gcsim.wasm"
              "GCSIM_HOST=0.0.0.0"
              "GCSIM_PORT=8080"
              "MAX_FUEL_VALIDATE=50M"
              "MAX_FUEL_SAMPLE=500M"
              "MAX_MEMORY=256Mi"
              "MAX_CONCURRENT_SIMS=4"
            ];
            ExposedPorts = {
              "8080/tcp" = { };
            };
          };
        };

      in
      {
        packages = {
          default = gcsimMcpServer;
          wasm = gcsimWasm;
          server = gcsimMcpServer;
          docker = dockerImage;
        };

        # 開発用シェル
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            go
            rustc
            cargo
            rustfmt
            clippy
            wasmtime
          ];
        };
      }
    );
}
```

### ビルド & デプロイコマンド

1. **コンテナイメージのビルド**:
   ```bash
   nix build .#docker
   ```
2. **Docker へのロード & 実行**:
   ```bash
   docker load < result
   docker run -p 8080:8080 gcsim-mcp:latest
   ```
3. **ローカル開発（stdio起動）**:
   ```bash
   nix build .#wasm
   nix build .#server
   ./result/bin/gcsim-mcp-server --stdio --wasm-path ./result-bin/share/gcsim.wasm
   ```
