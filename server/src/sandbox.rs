use anyhow::{Result, anyhow};
use std::sync::Arc;
use wasmtime::*;
use wasmtime_wasi::pipe::{MemoryInputPipe, MemoryOutputPipe};
use wasmtime_wasi::preview1::{self, WasiP1Ctx};

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
        current: u32,
        desired: u32,
        maximum: Option<u32>,
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
        tracing::info!(
            "Loading and JIT-compiling Wasm module from '{}'...",
            wasm_path
        );
        let start = std::time::Instant::now();

        let mut config = Config::new();
        config.consume_fuel(true);

        let engine = Engine::new(&config)?;
        let module = Module::from_file(&engine, wasm_path)?;
        tracing::info!("Wasm module compiled successfully in {:?}", start.elapsed());

        let mut linker = Linker::new(&engine);
        preview1::add_to_linker_sync(&mut linker, |state: &mut HostState| &mut state.wasi)?;

        Ok(Self {
            engine,
            module,
            linker: Arc::new(linker),
            max_memory_bytes,
        })
    }

    pub async fn execute(&self, input_json: &str, max_fuel: u64) -> Result<String> {
        let sandbox = self.clone();
        let input_bytes = input_json.as_bytes().to_vec();

        tokio::task::spawn_blocking(move || sandbox.execute_sync(input_bytes, max_fuel))
            .await
            .map_err(|e| anyhow!("Wasm execution task failed: {}", e))?
    }

    fn execute_sync(&self, input_bytes: Vec<u8>, max_fuel: u64) -> Result<String> {
        let stdin = MemoryInputPipe::new(input_bytes);
        let stdout = MemoryOutputPipe::new(10 * 1024 * 1024); // 最大10MB出力バッファ
        let stderr = MemoryOutputPipe::new(10 * 1024 * 1024); // 最大10MBエラーバッファ

        let wasi_ctx = wasmtime_wasi::WasiCtxBuilder::new()
            .stdin(stdin)
            .stdout(stdout.clone())
            .stderr(stderr.clone())
            .args(&["gcsim.wasm"])
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

        let instance = self.linker.instantiate(&mut store, &self.module)?;
        let start_fn = instance.get_typed_func::<(), ()>(&mut store, "_start")?;
        let res = start_fn.call(&mut store, ());

        if let Err(e) = res {
            if let Some(exit) = e.downcast_ref::<wasmtime_wasi::I32Exit>() {
                if exit.0 != 0 {
                    let stderr_bytes = stderr.contents();
                    let stderr_str = String::from_utf8_lossy(&stderr_bytes);
                    return Err(anyhow!(
                        "Wasm exited with non-zero status {}: {}",
                        exit.0,
                        stderr_str.trim()
                    ));
                }
                // exit.0 == 0 means normal program exit via proc_exit(0)
            } else {
                let stderr_bytes = stderr.contents();
                let stderr_str = String::from_utf8_lossy(&stderr_bytes);
                if !stderr_str.is_empty() {
                    tracing::error!("WASI stderr output:\n{}", stderr_str);
                }

                if matches!(e.downcast_ref::<Trap>(), Some(Trap::OutOfFuel)) {
                    return Err(anyhow!(
                        "Instruction fuel limit exceeded (max_fuel={}). Potential infinite loop or overly large simulation.",
                        max_fuel
                    ));
                }
                if !stderr_str.is_empty() {
                    return Err(anyhow!("{}: {}", e, stderr_str.trim()));
                }
                return Err(e);
            }
        }

        let output_bytes = stdout.contents();
        let output_str = String::from_utf8(output_bytes.into())?;
        Ok(output_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wasm_execution() {
        let wasm_path = std::env::var("WASM_PATH")
            .unwrap_or_else(|_| panic!("WASM_PATH environment variable not set"));
        if !std::path::Path::new(&wasm_path).exists() {
            println!("Skipping test, gcsim.wasm not found");
            return;
        }

        let sandbox =
            WasmSandbox::new(&wasm_path, 256 * 1024 * 1024).expect("Failed to create sandbox");
        let req = serde_json::json!({
            "action": "reference",
            "category": "characters"
        })
        .to_string();

        let res = sandbox.execute(&req, 50_000_000).await;
        match res {
            Ok(out) => println!(
                "Reference output: {}",
                &out[..std::cmp::min(out.len(), 200)]
            ),
            Err(e) => panic!("Execute failed: {:?}", e),
        }

        let cfg = r#"
options iteration=1 duration=5;
target lvl=100 hp=100000000 pos=0,2 resist=0.1;
raiden char lvl=90/90 cons=0 talent=9,9,10;
raiden add weapon="engulfinglightning" refine=1 lvl=90/90;
raiden add set="emblemofseveredfate" count=4;
raiden add stats hp=4780 atk=311 er=0.518 electro%=0.466 cr=0.311;
active raiden;
raiden attack;
"#;

        let val_req = serde_json::json!({
            "action": "validate",
            "config": cfg
        })
        .to_string();
        let val_res = sandbox
            .execute(&val_req, 50_000_000)
            .await
            .expect("validate failed");
        println!("Validate output: {}", val_res);

        let sample_req = serde_json::json!({
            "action": "sample",
            "config": cfg,
            "seed": 12345
        })
        .to_string();
        let sample_res = sandbox
            .execute(&sample_req, 500_000_000)
            .await
            .expect("sample failed");
        println!(
            "Sample output: {}",
            &sample_res[..std::cmp::min(sample_res.len(), 300)]
        );
    }
}
