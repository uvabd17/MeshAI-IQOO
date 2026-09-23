//! Owns the llama.cpp child processes: the host `llama-server` (when the laptop is host), the
//! local `ggml-rpc-server` (laptop-as-worker for a phone host) and simulated-phone workers.
//!
//! llama.cpp semantics the args depend on (verified against `load_tensors: layer N assigned to
//! device` debug output on 2026-09-24, see docs/BENCHMARKS.md):
//! * `--rpc a:p,b:p` registers remote devices, in that order, ahead of the CPU;
//! * llama.cpp counts the output head as layer `n_layer`, so `-ngl N` offloads the last N of
//!   (n_layer + 1) entries. We pass `ngl + 1` so exactly `ngl` real layers move, and pin
//!   `output.weight` back to the host with `--override-tensor`;
//! * `--tensor-split f1,f2` places offloaded entries by cumulative fraction, so worker k gets
//!   `layers_k / (ngl+1)` and the last worker one extra slot for the head entry.
//!
//! Lifecycle: `start` returns as soon as the plan is recorded and the background task is spawned;
//! the task is tagged with a generation number and stops touching state once `stop` bumps it
//! (N5: Stop is never blocked behind a slow start).

use crate::state::{now_ms, AppState};
use meshcore::planner::{Mode, Plan, Role};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

#[derive(Debug, serde::Serialize)]
pub struct LlamaArgs {
    pub program: String,
    pub args: Vec<String>,
    pub host_is_local: bool,
    pub workers: Vec<(String, u16)>,
    pub ngl: u32,
}

/// Pure derivation of llama-server arguments from a plan (unit-tested below).
pub fn derive_args(
    model_path: &str,
    n_ctx: u32,
    mode: &Mode,
    worker_layers: &[u32],
    worker_addrs: &[(String, u16)],
) -> (Vec<String>, u32) {
    let ngl: u32 = worker_layers.iter().sum();
    let mut args = vec![
        "-m".into(),
        model_path.to_string(),
        "-c".into(),
        n_ctx.to_string(),
        "--host".into(),
        "127.0.0.1".into(),
        "--port".into(),
        meshcore::HOST_LLAMA_PORT.to_string(),
        "--jinja".into(),
        "--metrics".into(),
        // Demo default: answer directly (Qwen3 thinking off, D017).
        "--reasoning".into(),
        "off".into(),
    ];
    if *mode == Mode::LayerSplit && !worker_addrs.is_empty() {
        args.push("--rpc".into());
        args.push(
            worker_addrs
                .iter()
                .map(|(a, p)| format!("{a}:{p}"))
                .collect::<Vec<_>>()
                .join(","),
        );
        args.push("-ngl".into());
        args.push((ngl + 1).to_string());
        args.push("--override-tensor".into());
        args.push("output\\.weight=CPU".into());
        if worker_addrs.len() > 1 {
            let total = (ngl + 1) as f64;
            let split: Vec<String> = worker_layers
                .iter()
                .enumerate()
                .map(|(i, &n)| {
                    let n = if i + 1 == worker_layers.len() {
                        n + 1
                    } else {
                        n
                    };
                    format!("{:.6}", n as f64 / total)
                })
                .collect();
            args.push("--tensor-split".into());
            args.push(split.join(","));
        }
    } else {
        args.push("-ngl".into());
        args.push("0".into());
    }
    (args, ngl)
}

/// Address of the laptop as the host device must reach it: our end of the host's control link.
fn local_addr_for(st: &AppState, host_id: &str) -> String {
    st.devices
        .read()
        .unwrap()
        .get(host_id)
        .and_then(|d| d.link_local_addr.clone())
        .or_else(|| local_ip_address::local_ip().ok().map(|i| i.to_string()))
        .unwrap_or_else(|| "127.0.0.1".into())
}

pub fn llama_args(st: &AppState, plan: &Plan) -> anyhow::Result<LlamaArgs> {
    let model = st
        .model(&plan.model)
        .ok_or_else(|| anyhow::anyhow!("model missing"))?;
    let host_is_local;
    let mut workers: Vec<(String, u16)> = Vec::new();
    let mut layer_counts: Vec<u32> = Vec::new();
    {
        let devices = st.devices.read().unwrap();
        let host = devices
            .get(&plan.host_id)
            .ok_or_else(|| anyhow::anyhow!("host device missing"))?;
        host_is_local = host.is_local;
        for p in plan.placements.iter().filter(|p| p.role == Role::Worker) {
            let d = devices
                .get(&p.device_id)
                .ok_or_else(|| anyhow::anyhow!("worker device missing"))?;
            let addr = if d.is_local {
                // The laptop worker binds to the host phone's link address (N2/H2).
                host.link_local_addr
                    .clone()
                    .unwrap_or_else(|| "127.0.0.1".into())
            } else {
                d.addr.clone().unwrap_or_default()
            };
            workers.push((addr, d.rpc_port));
            layer_counts.push(p.layer_end - p.layer_start);
        }
    }
    let (args, ngl) = derive_args(
        &st.models_dir.join(&model.file).display().to_string(),
        plan.n_ctx,
        &plan.mode,
        &layer_counts,
        &workers,
    );
    Ok(LlamaArgs {
        program: st.llama_bin.join("llama-server").display().to_string(),
        args,
        host_is_local,
        workers,
        ngl,
    })
}

pub fn spawn_rpc_server(
    bin: &Path,
    host: &str,
    port: u16,
    threads: Option<usize>,
    cache: bool,
) -> anyhow::Result<Child> {
    // NB: upstream ggml-rpc-server flags are only -t/-d/-H/-p/-c (no memory cap); limits live in the planner.
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

fn set_error(st: &AppState, msg: &str) {
    st.push_log(msg.to_string());
    let mut r = st.run.write().unwrap();
    r.status = "error".into();
    r.error = Some(msg.to_string());
}

/// Record the plan and launch the background bring-up task. Returns immediately (status
/// `starting`). Callers hold `st.run_lock` only across this call.
pub async fn start(st: Arc<AppState>, plan: Plan) -> anyhow::Result<()> {
    stop(&st).await;
    let gen = st.run_gen.fetch_add(1, Ordering::SeqCst) + 1;
    let plan_id = format!("plan-{}", now_ms());
    st.set_roles_from_plan(&plan);
    *st.plan.write().unwrap() = Some(plan.clone());
    {
        let mut r = st.run.write().unwrap();
        *r = crate::state::RunStatus {
            status: "starting".into(),
            host_id: Some(plan.host_id.clone()),
            model: Some(plan.model.clone()),
            plan_id: Some(plan_id.clone()),
            started_ms: Some(now_ms()),
            ..Default::default()
        };
    }
    let la = match llama_args(&st, &plan) {
        Ok(la) => la, // fail fast on a broken plan, and say so in the status
        Err(e) => {
            set_error(&st, &format!("cannot derive llama-server arguments: {e}"));
            *st.plan.write().unwrap() = None;
            return Err(e);
        }
    };
    st.run.write().unwrap().args = std::iter::once(la.program.clone())
        .chain(la.args.iter().cloned())
        .collect();
    tracing::info!(
        "start {plan_id} gen {gen}: {} mode {:?} host {}",
        plan.model,
        plan.mode,
        plan.host_id
    );
    let st2 = st.clone();
    tokio::spawn(async move {
        if let Err(e) = bring_up(st2.clone(), plan, plan_id, gen, la).await {
            fail_if_current(&st2, gen, &format!("start failed: {e}")).await;
        }
    });
    Ok(())
}

macro_rules! bail_if_stale {
    ($st:expr, $gen:expr) => {
        if $st.gen() != $gen {
            anyhow::bail!("cancelled");
        }
    };
}

/// Mark a device as holding a plan and push it. Caller holds `proc_lock` and has checked the
/// generation, so a concurrent stop can never race the registration (round-3 #3).
fn push_plan_locked(st: &AppState, id: &str, plan: meshcore::proto::Plan) {
    if let Some(dev) = st.devices.write().unwrap().get_mut(id) {
        dev.worker_ready_plan = None;
        dev.has_plan = true;
        dev.last_plan = Some(plan.clone());
    }
    let _ = st.plan_tx.send((id.to_string(), plan));
}

/// End the run with an error, but only if it is still generation `gen`: a newer run is never touched.
async fn fail_if_current(st: &Arc<AppState>, gen: u64, msg: &str) {
    let _g = st.proc_lock.lock().await;
    if st.gen() != gen {
        tracing::info!("gen {gen} superseded: {msg}");
        return;
    }
    set_error(st, msg);
    withdraw_and_kill_locked(st).await;
    let mut r = st.run.write().unwrap();
    r.endpoint = None;
    r.plan_id = None;
    drop(r);
    *st.plan.write().unwrap() = None;
}

/// A device reported that its process died or that it refused the plan: end the current run if
/// that device is part of it (round-3 #2).
pub async fn fail_run_from_device(st: &Arc<AppState>, device_id: &str, plan_id: &str, why: &str) {
    let gen = st.gen();
    let involved = st
        .devices
        .read()
        .unwrap()
        .get(device_id)
        .map(|d| d.has_plan)
        .unwrap_or(false);
    // A report is only honoured for the plan it belongs to: a stale "download cancelled" from the
    // run we just superseded must never kill the new one (round-4 #1).
    let current = st.run.read().unwrap().plan_id.clone();
    if !involved || current.as_deref() != Some(plan_id) {
        tracing::info!(
            "ignoring failure report from {device_id} for plan {plan_id:?} (current {current:?}, in run: {involved}): {why}"
        );
        return;
    }
    fail_if_current(st, gen, &format!("device {device_id}: {why}")).await;
}

/// Forget a device (admin action): if it is part of the current run, end the run; tell its live
/// session to say Bye and close; drop it from the device table and the pairing book (round-4 #5).
pub async fn forget_device(st: &Arc<AppState>, id: &str) {
    let _g = st.run_lock.lock().await;
    let in_run = st
        .devices
        .read()
        .unwrap()
        .get(id)
        .map(|d| d.has_plan || d.role == "host" || d.role == "worker")
        .unwrap_or(false);
    if in_run {
        st.push_log(format!("device {id} forgotten during the run — stopping"));
        stop(st).await;
        let mut r = st.run.write().unwrap();
        r.status = "error".into();
        r.error = Some(format!("device {id} was forgotten"));
    }
    let _ = st.session_ctl.send((id.to_string(), 0));
    st.devices.write().unwrap().remove(id);
    st.pairing.lock().unwrap().forget(id);
    st.save_paired();
}

async fn bring_up(
    st: Arc<AppState>,
    plan: Plan,
    plan_id: String,
    gen: u64,
    la: LlamaArgs,
) -> anyhow::Result<()> {
    // Local worker (laptop as compute for a phone host): bind to our end of the host's link.
    if plan
        .placements
        .iter()
        .any(|p| p.role == Role::Worker && p.device_id == "local")
    {
        let bind = local_addr_for(&st, &plan.host_id);
        let _g = st.proc_lock.lock().await;
        bail_if_stale!(st, gen);
        let child = spawn_rpc_server(&st.llama_bin, &bind, meshcore::RPC_PORT, None, true)?;
        st.push_log(format!(
            "local worker: ggml-rpc-server on {bind}:{} (pid {})",
            meshcore::RPC_PORT,
            child.id().unwrap_or(0)
        ));
        *st.local_worker.lock().unwrap() = Some(child);
        *st.local_worker_bind.lock().unwrap() = Some(bind);
    }

    // Push the plan to remote *workers* first (each gets the laptop's address as it sees it).
    let remote_workers: Vec<String> = plan
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
    {
        let _g = st.proc_lock.lock().await;
        bail_if_stale!(st, gen);
        for id in &remote_workers {
            push_plan_locked(&st, id, to_proto(&st, &plan, &plan_id, id));
        }
    }
    // Remote phones report "worker listening" for this plan id (M2): a bare TCP probe is not
    // enough because adb/port forwards accept connections before anything listens behind them.
    for id in &remote_workers {
        let mut ok = false;
        for _ in 0..90 {
            bail_if_stale!(st, gen);
            let ready = st
                .devices
                .read()
                .unwrap()
                .get(id)
                .map(|d| d.worker_ready_plan.as_deref() == Some(plan_id.as_str()))
                .unwrap_or(false);
            if ready {
                ok = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        if ok {
            st.push_log(format!("worker ready on device {id}"));
        } else {
            anyhow::bail!("device {id} never reported its RPC worker listening (45 s) — is the app in the foreground?");
        }
    }
    // llama.cpp aborts the whole process if an RPC server is unreachable, so TCP-check every worker.
    for (addr, port) in &la.workers {
        let target = format!("{addr}:{port}");
        let mut ok = false;
        for _ in 0..40 {
            bail_if_stale!(st, gen);
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
            anyhow::bail!("worker {target} not reachable after 20 s — is its RPC server running on the paired link?");
        }
    }
    bail_if_stale!(st, gen);

    if la.host_is_local {
        st.push_log(format!("host: {} {}", la.program, la.args.join(" ")));
        let mut cmd = Command::new(&la.program);
        cmd.args(&la.args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = {
            let _g = st.proc_lock.lock().await;
            bail_if_stale!(st, gen);
            let child = cmd
                .spawn()
                .map_err(|e| anyhow::anyhow!("spawn llama-server: {e}"))?;
            *st.host_pid.lock().unwrap() = child.id();
            st.run.write().unwrap().status = "loading".into(); // inside the lock (round-4 #7)
            child
        };
        let stderr = child.stderr.take();
        let stdout = child.stdout.take();
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
        // Watch the child for its whole life (H6): an exit after "ready" is an error, not silence.
        let st2 = st.clone();
        tokio::spawn(async move {
            let status = child.wait().await;
            let msg = match status {
                Ok(c) => format!("llama-server exited ({c})"),
                Err(e) => format!("llama-server wait failed: {e}"),
            };
            {
                let _g = st2.proc_lock.lock().await;
                if st2.gen() != gen {
                    return; // a newer run owns the state now
                }
                *st2.host_pid.lock().unwrap() = None;
                let s = st2.run.read().unwrap().status.clone();
                if !(s == "loading" || s == "ready" || s == "starting") {
                    return;
                }
            }
            fail_if_current(&st2, gen, &msg).await;
        });
        let st2 = st.clone();
        tokio::spawn(async move {
            wait_ready(
                st2,
                gen,
                format!("http://127.0.0.1:{}", meshcore::HOST_LLAMA_PORT),
            )
            .await
        });
    } else {
        // Phone host: push its plan only now that its workers are up; it runs llama-server on :8081.
        {
            let _g = st.proc_lock.lock().await;
            bail_if_stale!(st, gen);
            push_plan_locked(
                &st,
                &plan.host_id,
                to_proto(&st, &plan, &plan_id, &plan.host_id),
            );
            st.run.write().unwrap().status = "loading".into(); // inside the lock (round-4 #7)
        }
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
        let st2 = st.clone();
        tokio::spawn(async move { wait_ready(st2, gen, ep).await });
    }
    Ok(())
}

async fn wait_ready(st: Arc<AppState>, gen: u64, endpoint: String) {
    let client = reqwest::Client::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20 * 60);
    loop {
        if st.gen() != gen || st.run.read().unwrap().status != "loading" {
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
                if run.status != "loading" || st.gen() != gen {
                    return;
                }
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
        if std::time::Instant::now() > deadline {
            fail_if_current(&st, gen, "timeout waiting for /health").await;
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

async fn kill_pid_and_wait(pid: u32) {
    let _ = Command::new("kill").arg(pid.to_string()).output().await;
    for _ in 0..30 {
        if !Path::new(&format!("/proc/{pid}")).exists() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let _ = Command::new("kill")
        .arg("-9")
        .arg(pid.to_string())
        .output()
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}

/// Kill the host llama-server and the local worker and wait for them to exit (M10).
async fn stop_processes(st: &Arc<AppState>) {
    let pid = st.host_pid.lock().unwrap().take();
    if let Some(pid) = pid {
        kill_pid_and_wait(pid).await;
    }
    let lw = st.local_worker.lock().unwrap().take();
    if let Some(mut c) = lw {
        let _ = c.start_kill();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(3), c.wait()).await;
    }
    *st.local_worker_bind.lock().unwrap() = None;
}

/// Withdraw the plan from every remote device that received one (they kill their processes) and
/// kill ours. Caller holds `proc_lock`. Used by stop, the host watcher and a failed bring-up (H6).
async fn withdraw_and_kill_locked(st: &Arc<AppState>) {
    stop_processes(st).await;
    let ids: Vec<String> = st
        .devices
        .read()
        .unwrap()
        .values()
        .filter(|d| !d.is_local && d.has_plan)
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
        dev.has_plan = false;
        dev.worker_ready_plan = None;
        dev.last_plan = None;
    }
}

pub async fn stop(st: &Arc<AppState>) {
    let _g = st.proc_lock.lock().await;
    st.run_gen.fetch_add(1, Ordering::SeqCst); // cancels any bring-up / watcher of the old run
    withdraw_and_kill_locked(st).await;
    let mut r = st.run.write().unwrap();
    r.status = "idle".into();
    r.endpoint = None;
    r.plan_id = None;
    *st.plan.write().unwrap() = None;
}

/// Plan as sent to `for_device`: the laptop's address is our end of *that* device's link.
pub fn to_proto(
    st: &AppState,
    plan: &Plan,
    plan_id: &str,
    for_device: &str,
) -> meshcore::proto::Plan {
    use meshcore::proto as p;
    let my_ip = local_addr_for(st, for_device);
    let devices = st.devices.read().unwrap();
    p::Plan {
        plan_id: plan_id.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn arg_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        args.iter()
            .position(|a| a == flag)
            .map(|i| args[i + 1].as_str())
    }

    #[test]
    fn single_device_keeps_everything_on_host() {
        let (args, ngl) = derive_args("m.gguf", 2048, &Mode::Single, &[], &[]);
        assert_eq!(ngl, 0);
        assert_eq!(arg_after(&args, "-ngl"), Some("0"));
        assert!(!args.iter().any(|a| a == "--rpc"));
        assert_eq!(arg_after(&args, "--reasoning"), Some("off"));
    }

    #[test]
    fn split_offloads_exactly_the_worker_layers_and_pins_the_head() {
        // Qwen3-0.6B: 28 layers; host 0-3, w1 4-14 (11), w2 15-27 (13). Verified against llama.cpp
        // `layer N assigned to device` output: CPU 0-3, RPC0 4-14, RPC1 15-27 (+ head pinned to CPU).
        let workers = vec![
            ("10.0.0.2".to_string(), 50052u16),
            ("10.0.0.3".to_string(), 50052),
        ];
        let (args, ngl) = derive_args("m.gguf", 2048, &Mode::LayerSplit, &[11, 13], &workers);
        assert_eq!(ngl, 24);
        assert_eq!(
            arg_after(&args, "-ngl"),
            Some("25"),
            "ngl+1: the output head counts as a layer"
        );
        assert_eq!(
            arg_after(&args, "--override-tensor"),
            Some("output\\.weight=CPU")
        );
        assert_eq!(
            arg_after(&args, "--rpc"),
            Some("10.0.0.2:50052,10.0.0.3:50052")
        );
        // 11/25 and (13+1)/25
        assert_eq!(
            arg_after(&args, "--tensor-split"),
            Some("0.440000,0.560000")
        );
    }

    #[test]
    fn single_worker_has_no_tensor_split() {
        let workers = vec![("10.0.0.2".to_string(), 50052u16)];
        let (args, ngl) = derive_args("m.gguf", 4096, &Mode::LayerSplit, &[22], &workers);
        assert_eq!(ngl, 22);
        assert_eq!(arg_after(&args, "-ngl"), Some("23"));
        assert!(!args.iter().any(|a| a == "--tensor-split"));
    }
}
