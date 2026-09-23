//! meshd — MeshAI coordinator.
//!
//! `meshd serve`   control plane (:7070) + API/admin/proxy (:8080, localhost unless --lan)
//! `meshd mirror`  internet-facing read-only admin fed by a coordinator's `--push-to`
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
    /// Where runs/pairing state are persisted
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
        /// Bind the API/admin to all interfaces (needed for a phone host to fetch models). Requires --api-token.
        #[arg(long, default_value_t = false)]
        lan: bool,
        /// Token required on mutating API routes and /v1 (header x-mesh-token or Bearer).
        #[arg(long, env = "MESHAI_API_TOKEN")]
        api_token: Option<String>,
        /// Mirror this coordinator's live state to a cloud `meshd mirror` (e.g. https://mesh.example.com)
        #[arg(long, env = "MESHAI_PUSH_TO")]
        push_to: Option<String>,
        #[arg(long, env = "MESHAI_PUSH_TOKEN")]
        push_token: Option<String>,
    },
    /// Serve the admin panel read-only from state pushed by a coordinator. Token from $MESHAI_MIRROR_TOKEN.
    Mirror {
        #[arg(long, default_value_t = meshcore::API_PORT)]
        api_port: u16,
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

fn new_state(cli: &Cli, lan: bool, api_token: Option<String>) -> Arc<state::AppState> {
    Arc::new(state::AppState::new(
        cli.models.clone(),
        cli.llama_bin.clone(),
        cli.state_dir.clone(),
        lan,
        api_token,
    ))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("meshd=info".parse()?),
        )
        .init();
    let cli = Cli::parse();

    match &cli.cmd {
        Cmd::Serve {
            api_port,
            control_port,
            lan,
            api_token,
            push_to,
            push_token,
        } => {
            if *lan && api_token.is_none() {
                anyhow::bail!(
                    "--lan exposes the API to the network: set --api-token (or MESHAI_API_TOKEN)"
                );
            }
            let st = new_state(&cli, *lan, api_token.clone());
            st.refresh_local_profile();
            st.scan_models();
            if let (Some(u), Some(t)) = (push_to, push_token) {
                if !u.starts_with("https://") {
                    tracing::warn!("mirror push over plain HTTP: the relay token travels in cleartext (fine on a trusted LAN, not on the internet)");
                }
                *st.push_to.write().unwrap() =
                    Some((u.trim_end_matches('/').to_string(), t.clone()));
                tokio::spawn(api::push_loop(st.clone()));
            }
            let c = control::serve(st.clone(), *control_port);
            let a = api::serve(st.clone(), *api_port, false);
            let host = if *lan { "<this-ip>" } else { "127.0.0.1" };
            tracing::info!("admin: http://{host}:{api_port}/admin  · OpenAI API: http://{host}:{api_port}/v1  · control: :{control_port}");
            tokio::try_join!(c, a)?;
        }
        Cmd::Mirror { api_port } => {
            let token = std::env::var("MESHAI_MIRROR_TOKEN")
                .ok()
                .filter(|t| t.len() >= 16)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "MESHAI_MIRROR_TOKEN (>= 16 chars) must be set in the environment"
                    )
                })?;
            let st = new_state(&cli, true, None);
            *st.mirror_token.write().unwrap() = Some(token);
            tracing::info!("mirror mode: read-only admin at :{api_port}/admin, waiting for a coordinator to push state");
            api::serve(st.clone(), *api_port, true).await?;
        }
        Cmd::Pair => {
            let st = new_state(&cli, false, None);
            let offer = st.new_offer(meshcore::CONTROL_PORT);
            println!("{}", state::qr_terminal(&offer.qr_payload()));
            println!("{}", offer.qr_payload());
        }
        Cmd::Plan { model, n_ctx, sim } => {
            let st = new_state(&cli, false, None);
            st.refresh_local_profile();
            st.scan_models();
            if let Some(s) = sim {
                for (i, gb) in s.split(',').enumerate() {
                    let gb: f64 = gb.trim().parse().unwrap_or(8.0);
                    st.add_sim_device(&format!("sim-phone-{}", i + 1), (gb * 1e9) as u64, 6.0);
                }
            }
            let plan = st.make_plan(model, *n_ctx, None)?;
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
            let bind = local_ip_address::local_ip()
                .map(|i| i.to_string())
                .unwrap_or_else(|_| "127.0.0.1".into());
            let child = supervisor::spawn_rpc_server(&cli.llama_bin, &bind, *port, *threads, true)?;
            tracing::info!(
                "worker: ggml-rpc-server listening on {bind}:{port} (pid {})",
                child.id().unwrap_or(0)
            );
            tokio::signal::ctrl_c().await?;
        }
    }
    Ok(())
}
