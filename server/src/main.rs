use anyhow::Result;
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtx, WasiCtxView, WasiView};

wasmtime::component::bindgen!({
    world: "executor-world",
    path: "../executor/wit",
    exports: { default: async }
});

const EXECUTOR_WASM: &[u8] = include_bytes!("../../target/wasm32-wasip2/release/executor.wasm");

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The script file to execute
    file: Option<String>,

    #[arg(
        short,
        long,
        help = "The script to execute inline",
        conflicts_with = "file"
    )]
    command: Option<String>,

    #[arg(short, help = "Mapping of host paths to guest paths")]
    mount: Option<Vec<DirMount>>,
}

#[derive(Debug, Clone)]

struct DirMount {
    host: PathBuf,
    guest: PathBuf,
    dir_perms: DirPerms,
    file_perms: FilePerms,
}

impl FromStr for DirMount {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let parts = s.splitn(3, ':').collect::<Vec<&str>>();
        let (dir_perms, file_perms) = match parts.get(2) {
            Some(&"ro") => (DirPerms::READ, FilePerms::READ),
            Some(&"rw") => (DirPerms::MUTATE, FilePerms::READ | FilePerms::WRITE),
            Some(_) => anyhow::bail!("Invalid permissions: {}", parts[2]),
            None => (DirPerms::READ, FilePerms::READ),
        };

        Ok(DirMount {
            host: PathBuf::from(parts[0]),
            guest: PathBuf::from(parts[1]),
            dir_perms,
            file_perms,
        })
    }
}

struct MyState {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl WasiView for MyState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

async fn handle_request(
    wasi_ctx: WasiCtx,
    script: String,
    component: Component,
    engine: Engine,
) -> Result<String> {
    let state = MyState {
        ctx: wasi_ctx,
        table: ResourceTable::new(),
    };

    let mut store = Store::new(&engine, state);

    let mut linker = Linker::new(&engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;

    let executor = ExecutorWorld::instantiate_async(&mut store, &component, &linker).await?;
    match executor.call_run(&mut store, &script).await? {
        Ok(output) => Ok(output),
        Err(e) => Err(anyhow::anyhow!(e.to_string())),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let script = if let Some(command) = args.command {
        command
    } else if let Some(file_path) = args.file {
        fs::read_to_string(file_path)?
    } else {
        anyhow::bail!("Please provide a script file or an inline command using -c.");
    };

    let mut config = Config::new();
    config.async_support(true);

    let engine = Engine::new(&config)?;
    let component = Component::from_binary(&engine, EXECUTOR_WASM)?;

    let mut wasi_ctx = WasiCtx::builder();
    if let Some(mounts) = args.mount {
        for mount in mounts {
            wasi_ctx.preopened_dir(
                mount.host,
                mount.guest.to_str().unwrap(),
                mount.dir_perms,
                mount.file_perms,
            )?;
        }
    }

    let wasi_ctx = wasi_ctx.inherit_stdio().build();
    match handle_request(wasi_ctx, script, component.clone(), engine.clone()).await {
        Ok(output) => println!("Output: {}", output),
        Err(e) => println!("Error: {}", e),
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use wasmtime_wasi::{DirPerms, FilePerms};

    use crate::DirMount;
    use std::path::PathBuf;

    #[test]
    fn test_parse_mounts() {
        let mount_ro: DirMount = "a:b:ro".parse().unwrap();
        assert_eq!(mount_ro.host, PathBuf::from("a"));
        assert_eq!(mount_ro.guest, PathBuf::from("b"));
        assert_eq!(mount_ro.dir_perms, DirPerms::READ);
        assert_eq!(mount_ro.file_perms, FilePerms::READ);
    }
}
