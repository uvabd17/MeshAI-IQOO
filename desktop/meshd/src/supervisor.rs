//! Owns the llama.cpp child processes: the host `llama-server` (when the laptop is host),
//! local `ggml-rpc-server` workers (laptop-as-worker or simulated phones).
//!
//! llama.cpp semantics that the args below depend on:
//! * `--rpc a:p,b:p` registers remote devices in that order;
//! * `-ngl N` offloads the *last* N transformer layers to those devices, host CPU keeps the rest
//!   plus embeddings/output;
//! * `--tensor-split w1,w2` divides the offloaded layers among the RPC devices in `--rpc` order.

use crate::state::{now_ms, AppState};
use meshcore::planner::{Mode, Plan, Role};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Debug, serde::Serialize)]
pub struct LlamaArgs {
    pub program: String,
    pub args: Vec<String>,
    pub host_is_local: bool,
    pub workers: Vec<(String, u16)>,
}

pub fn llama_args(st: &AppState, plan: &Plan) -> anyhow::Result<LlamaArgs> {
    let model = st
        .model(&plan.model)
        .or_else(|| {
            st.models
                .read()
                .unwrap()
                .iter()
                .find(|m| m.info.name == plan.model)
                .cloned()
        })
        .ok_or_else(|| anyhow::anyhow!("model missing"))?;
    let devices = st.devices.read().unwrap();
    let host = devices
        .get(&plan.host_id)
        .ok_or_else(|| anyhow::anyhow!("host device missing"))?;
    let mut workers: Vec<(String, u16)> = Vec::new();
    let mut split: Vec<String> = Vec::new();
    let mut ngl = 0u32;
    for p in plan.placements.iter().filter(|p| p.role == Role::Worker) {
        let d = devices
            .get(&p.device_id)
            .ok_or_else(|| anyhow::anyhow!("worker device missing"))?;
        let addr = if d.is_local {
            "127.0.0.1".to_string()
        } else {
            d.addr.clone().unwrap_or_default()
        };
        workers.push((addr, d.rpc_port));
        split.push(format!("{:.4}", p.split_weight));
        ngl += p.layer_end - p.layer_start;
    }
    let mut args = vec![
        "-m".into(),
        st.models_dir.join(&model.file).display().to_string(),
        "-c".into(),
        plan.n_ctx.to_string(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        meshcore::HOST_LLAMA_PORT.to_string(),
        "--jinja".into(),
        "--metrics".into(),
        // Demo default: answer directly (Qwen3 thinking off). Per-request override is a Phase-2 knob.
        "--reasoning".into(),
        "off".into(),
    ];
    if plan.mode == Mode::LayerSplit && !workers.is_empty() {
        args.push("--rpc".into());
        args.push(
            workers
                .iter()
                .map(|(a, p)| format!("{a}:{p}"))
                .collect::<Vec<_>>()
                .join(","),
        );
        args.push("-ngl".into());
        args.push(ngl.to_string());
        if workers.len() > 1 {
            args.push("--tensor-split".into());
            args.push(split.join(","));
        }
    } else {
        args.push("-ngl".into());
        args.push("0".into());
    }
    Ok(LlamaArgs {
        program: st.llama_bin.join("llama-server").display().to_string(),
        args,
        host_is_local: host.is_local,
        workers,
    })
}

pub fn spawn_rpc_server(
    bin: &Path,
    host: &str,
    port: u16,
    threads: Option<usize>,
    cache: bool,
) -> anyhow::Result<Child> {
    // NB: upstream ggml-rpc-server flags are only -t/-d/-H/-p/-c (no memory cap); memory limits live in the planner.
    let mut cmd = Command::new(bin.join("ggml-rpc-server"));
    cmd.arg("-H").arg(host).arg("-p").arg(port.to_string());
    if cache {
        cmd.arg("-c"); // one rpc-server per machine only: two cache users on one host corrupt each other
    }
    if let Some(t) = threads {
        cmd.arg("-t").arg(t.to_string());
    }
    cmd.stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    Ok(cmd.spawn()?)
}

/// Start the plan: spawn local workers if any placement is a local worker, spawn the host
/// llama-server if the host is local, otherwise push the plan to the phone host.
pub async fn start(st: Arc<AppState>, plan: Plan) -> anyhow::Result<()> {
    stop(&st).await;
    st.set_roles_from_plan(&plan);
    *st.plan.write().unwrap() = Some(plan.clone());
    {
        let mut r = st.run.write().unwrap();
        *r = crate::state::RunStatus {
            status: "starting".into(),
            host_id: Some(plan.host_id.clone()),
            model: Some(plan.model.clone()),
            started_ms: Some(now_ms()),
            ..Default::default()
        };
    }
    tracing::info!(
        "start: plan {} mode {:?} host {}",
        plan.model,
        plan.mode,
        plan.host_id
    );
    let la = llama_args(&st, &plan)?;
    st.run.write().unwrap().args = std::iter::once(la.program.clone())
        .chain(la.args.iter().cloned())
        .collect();

    // Local worker (laptop as compute for a phone host)
    if plan
        .placements
        .iter()
        .any(|p| p.role == Role::Worker && p.device_id == "local")
    {
        let child = spawn_rpc_server(&st.llama_bin, "0.0.0.0", meshcore::RPC_PORT, None, true)?;
        st.push_log(format!(
            "local worker: ggml-rpc-server on :{} (pid {})",
            meshcore::RPC_PORT,
            child.id().unwrap_or(0)
        ));
        *st.local_worker.lock().unwrap() = Some(child);
    }
    // Push the plan to every remote participant (phones start rpc-server / llama-server themselves)
    {
        let mut d = st.devices.write().unwrap();
        for dev in d.values_mut() {
            if !dev.is_local && dev.kind != crate::state::DeviceKind::Sim {
                dev.worker_ready = false;
            }
        }
    }
    let pplan = to_proto(&st, &plan);
    for p in plan
        .placements
        .iter()
        .filter(|p| p.role != Role::Rejected && p.device_id != "local")
    {
        let _ = st.plan_tx.send((p.device_id.clone(), pplan.clone()));
    }

    // llama.cpp aborts the whole process if an RPC server is unreachable, so wait for every worker first.
    tracing::info!("start: waiting for {} worker(s)", la.workers.len());
    // Remote phones report "worker listening" over the control plane first: a bare TCP probe is not
    // enough because adb/port forwards accept connections before anything listens behind them.
    let remote_ids: Vec<String> = plan
        .placements
        .iter()
        .filter(|p| p.role == Role::Worker && p.device_id != "local")
        .filter(|p| {
            st.devices
                .read()
                .unwrap()
                .get(&p.device_id)
                .map(|d| d.kind != crate::state::DeviceKind::Sim)
                .unwrap_or(false)
        })
        .map(|p| p.device_id.clone())
        .collect();
    for id in &remote_ids {
        let mut ok = false;
        for _ in 0..90 {
            if st
                .devices
                .read()
                .unwrap()
                .get(id)
                .map(|d| d.worker_ready)
                .unwrap_or(false)
            {
                ok = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        if ok {
            st.push_log(format!("worker ready on device {id}"));
        } else {
            let msg = format!("device {id} never reported its RPC worker listening (45 s) — is the app in the foreground?");
            st.push_log(msg.clone());
            let mut r = st.run.write().unwrap();
            r.status = "error".into();
            r.error = Some(msg.clone());
            anyhow::bail!(msg);
        }
    }
    for (addr, port) in &la.workers {
        let target = format!("{addr}:{port}");
        let mut ok = false;
        for _ in 0..40 {
            if tokio::time::timeout(
                std::time::Duration::from_millis(500),
                tokio::net::TcpStream::connect(&target),
            )
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
            {
                ok = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        if ok {
            st.push_log(format!("worker reachable: {target}"));
        } else {
            let msg = format!("worker {target} not reachable after 20 s — is its RPC server running on the paired link?");
            st.push_log(msg.clone());
            let mut r = st.run.write().unwrap();
            r.status = "error".into();
            r.error = Some(msg.clone());
            anyhow::bail!(msg);
        }
    }

    if la.host_is_local {
        st.push_log(format!("host: {} {}", la.program, la.args.join(" ")));
        let mut cmd = Command::new(&la.program);
        cmd.args(&la.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("spawn llama-server: {e}"))?;
        let stderr = child.stderr.take();
        let stdout = child.stdout.take();
        *st.child.lock().unwrap() = Some(child);
        st.run.write().unwrap().status = "loading".into();
        for pipe in [
            stdout.map(|p| Box::pin(p) as std::pin::Pin<Box<dyn tokio::io::AsyncRead + Send>>),
            stderr.map(|p| Box::pin(p) as _),
        ]
        .into_iter()
        .flatten()
        {
            let st2 = st.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(pipe).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    if l.contains("error") || l.contains("failed") {
                        tracing::warn!("llama-server: {l}");
                    }
                    st2.push_log(l);
                }
            });
        }
        let st2 = st.clone();
        tokio::spawn(async move {
            wait_ready(
                st2,
                format!("http://127.0.0.1:{}", meshcore::HOST_LLAMA_PORT),
            )
            .await
        });
    } else {
        // Phone host: llama-server runs there on HOST_LLAMA_PORT; we proxy to it.
        let addr = st
            .devices
            .read()
            .unwrap()
            .get(&plan.host_id)
            .and_then(|d| d.addr.clone())
            .unwrap_or_default();
        let ep = format!("http://{addr}:{}", meshcore::HOST_LLAMA_PORT);
        st.push_log(format!(
            "host is remote ({}): plan pushed, waiting for {ep}/health",
            plan.host_id
        ));
        st.run.write().unwrap().status = "loading".into();
        let st2 = st.clone();
        tokio::spawn(async move { wait_ready(st2, ep).await });
    }
    Ok(())
}

async fn wait_ready(st: Arc<AppState>, endpoint: String) {
    let client = reqwest::Client::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20 * 60);
    loop {
        if st.run.read().unwrap().status != "loading" {
            return;
        }
        if let Ok(r) = client
            .get(format!("{endpoint}/health"))
            .timeout(std::time::Duration::from_secs(2))
            .send()
            .await
        {
            if r.status().is_success() {
                let mut run = st.run.write().unwrap();
                run.status = "ready".into();
                run.ready_ms = Some(now_ms());
                run.endpoint = Some(endpoint.clone());
                let secs = (run.ready_ms.unwrap() - run.started_ms.unwrap_or(run.ready_ms.unwrap()))
                    as f64
                    / 1000.0;
                run.log_tail
                    .push(format!("ready in {secs:.1}s → {endpoint}"));
                return;
            }
        }
        // did the local child die?
        if let Some(c) = st.child.lock().unwrap().as_mut() {
            if let Ok(Some(code)) = c.try_wait() {
                let mut run = st.run.write().unwrap();
                run.status = "error".into();
                run.error = Some(format!("llama-server exited with {code}"));
                return;
            }
        }
        if std::time::Instant::now() > deadline {
            let mut run = st.run.write().unwrap();
            run.status = "error".into();
            run.error = Some("timeout waiting for /health".into());
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

pub async fn stop(st: &Arc<AppState>) {
    if let Some(mut c) = st.child.lock().unwrap().take() {
        let _ = c.start_kill();
    }
    if let Some(mut c) = st.local_worker.lock().unwrap().take() {
        let _ = c.start_kill();
    }
    // tell remote participants to stop (empty plan)
    let ids: Vec<String> = st
        .devices
        .read()
        .unwrap()
        .values()
        .filter(|d| !d.is_local && d.role != "idle")
        .map(|d| d.id.clone())
        .collect();
    for id in ids {
        let _ = st.plan_tx.send((
            id,
            meshcore::proto::Plan {
                plan_id: "stop".into(),
                ..Default::default()
            },
        ));
    }
    let mut d = st.devices.write().unwrap();
    for dev in d.values_mut() {
        dev.role = "idle".into();
    }
    let mut r = st.run.write().unwrap();
    r.status = "idle".into();
    r.endpoint = None;
    *st.plan.write().unwrap() = None;
}

pub fn to_proto(st: &AppState, plan: &Plan) -> meshcore::proto::Plan {
    use meshcore::proto as p;
    let devices = st.devices.read().unwrap();
    let my_ip = local_ip_address::local_ip()
        .map(|i| i.to_string())
        .unwrap_or_else(|_| "127.0.0.1".into());
    p::Plan {
        plan_id: format!("plan-{}", now_ms()),
        model_name: plan.model.clone(),
        model_sha: String::new(),
        mode: match plan.mode {
            Mode::Single => p::Mode::Single as i32,
            Mode::LayerSplit => p::Mode::LayerSplit as i32,
        },
        n_ctx: plan.n_ctx,
        placements: plan
            .placements
            .iter()
            .map(|x| {
                let d = devices.get(&x.device_id);
                p::Placement {
                    device_id: x.device_id.clone(),
                    used: x.role != Role::Rejected,
                    layer_start: x.layer_start,
                    layer_end: x.layer_end,
                    backend: p::Backend::Cpu as i32,
                    bytes: x.bytes,
                    reason: format!("{:?}: {}", x.role, x.reason),
                    addr: d
                        .and_then(|d| {
                            if d.is_local {
                                Some(my_ip.clone())
                            } else {
                                d.addr.clone()
                            }
                        })
                        .unwrap_or_default(),
                    rpc_port: d
                        .map(|d| d.rpc_port as u32)
                        .unwrap_or(meshcore::RPC_PORT as u32),
                    split_weight: x.split_weight,
                    is_host: x.role == Role::Host,
                }
            })
            .collect(),
        summary: plan.summary.clone(),
        model_file: plan.model.clone(),
        coordinator: format!("{my_ip}:{}", meshcore::API_PORT),
        n_threads: 0,
    }
}
