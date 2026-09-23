//! meshd — MeshAI coordinator.
//!
//! `meshd serve`   control plane (:7070) + API/admin/proxy (:8080)
//! `meshd pair`    print a pairing QR in the terminal
//! `meshd plan`    dry-run the planner for a model against the local device (+ simulated phones)
//! `meshd worker`  run this machine as a pure compute worker (ggml-rpc-server) for a phone host

mod api;
mod control;
mod models;
mod proxy;
mod state;
mod supervisor;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "meshd", version, about = "MeshAI coordinator")]
struct Cli {
    /// Directory with GGUF files (default $MESHAI_MODELS or /mnt/storage/meshai/models)
    #[arg(
        long,
        env = "MESHAI_MODELS",
        default_value = "/mnt/storage/meshai/models"
    )]
    models: PathBuf,
    /// Directory with llama.cpp host binaries (llama-server, ggml-rpc-server, llama-bench)
    #[arg(
        long,
        env = "MESHAI_LLAMA_BIN",
        default_value = "third_party/llama.cpp/build-host/bin"
    )]
    llama_bin: PathBuf,
    /// Where runs/telemetry are persisted
    #[arg(long, env = "MESHAI_STATE", default_value = "state")]
    state_dir: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    Serve {
        #[arg(long, default_value_t = meshcore::API_PORT)]
        api_port: u16,
        #[arg(long, default_value_t = meshcore::CONTROL_PORT)]
        control_port: u16,
    },
    Pair,
    Plan {
        model: String,
        #[arg(long, default_value_t = 4096)]
        n_ctx: u32,
        /// Add simulated phones, e.g. --sim 8.5,8.5 (usable GB each)
        #[arg(long)]
        sim: Option<String>,
    },
    Worker {
        #[arg(long, default_value_t = meshcore::RPC_PORT)]
        port: u16,
        #[arg(long)]
        threads: Option<usize>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("meshd=info".parse()?),
        )
        .init();
    let cli = Cli::parse();
    let st = Arc::new(state::AppState::new(
        cli.models.clone(),
        cli.llama_bin.clone(),
        cli.state_dir.clone(),
    ));

    match cli.cmd {
        Cmd::Serve {
            api_port,
            control_port,
        } => {
            st.refresh_local_profile();
            st.scan_models();
            let c = control::serve(st.clone(), control_port);
            let a = api::serve(st.clone(), api_port);
            tracing::info!("admin: http://127.0.0.1:{api_port}/admin  · OpenAI API: http://127.0.0.1:{api_port}/v1  · control: :{control_port}");
            tokio::try_join!(c, a)?;
        }
        Cmd::Pair => {
            let offer = st.new_offer(meshcore::CONTROL_PORT);
            println!("{}", state::qr_terminal(&offer.qr_payload()));
            println!("{}", offer.qr_payload());
        }
        Cmd::Plan { model, n_ctx, sim } => {
            st.refresh_local_profile();
            st.scan_models();
            if let Some(s) = sim {
                for (i, gb) in s.split(',').enumerate() {
                    let gb: f64 = gb.trim().parse().unwrap_or(8.0);
                    st.add_sim_device(&format!("sim-phone-{}", i + 1), (gb * 1e9) as u64, 6.0);
                }
            }
            let plan = st.make_plan(&model, n_ctx, None)?;
            println!("{}", plan.summary);
            for p in &plan.placements {
                println!("  {:<14} {:?}  {}", p.name, p.role, p.reason);
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&supervisor::llama_args(&st, &plan)?)?
            );
        }
        Cmd::Worker { port, threads } => {
            let child = supervisor::spawn_rpc_server(&cli.llama_bin, "0.0.0.0", port, threads, true)?;
            tracing::info!(
                "worker: ggml-rpc-server listening on :{port} (pid {})",
                child.id().unwrap_or(0)
            );
            tokio::signal::ctrl_c().await?;
        }
    }
    Ok(())
}
