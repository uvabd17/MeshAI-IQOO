//! JSON API for the admin panel + static admin files + /v1 proxy.

use crate::models;
use crate::state::{AppState, Device};
use crate::supervisor;
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use include_dir::{include_dir, Dir};
use serde::Deserialize;
use std::sync::Arc;

static ADMIN: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../admin/src");

pub async fn serve(st: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/", get(|| async { Redirect::temporary("/admin/") }))
        .route("/admin", get(|| async { Redirect::temporary("/admin/") }))
        .route("/admin/", get(admin_index))
        .route("/admin/{*path}", get(admin_static))
        .route("/api/state", get(api_state))
        .route("/api/relay/state", post(api_relay_state))
        .route("/api/catalog", get(api_catalog))
        .route("/api/models/rescan", post(api_rescan))
        .route("/api/models/download", post(api_download))
        .route("/api/models/file/{file}", get(api_model_file))
        .route("/api/pair/offer", post(api_offer))
        .route("/api/plan", post(api_plan))
        .route("/api/run", post(api_run))
        .route("/api/stop", post(api_stop))
        .route("/api/devices/{id}", delete(api_forget))
        .route("/api/devices/{id}/limit", post(api_limit))
        .route("/api/sim/workers", post(api_sim_workers))
        .route("/api/runs", get(api_runs))
        .route("/api/bench", post(api_bench))
        .route("/v1/{*rest}", axum::routing::any(crate::proxy::v1))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(st.clone());
    let l = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    // periodic local refresh + offline detection
    let st2 = st.clone();
    tokio::spawn(async move {
        loop {
            st2.refresh_local_profile();
            let now = crate::state::now_ms();
            {
                let mut d = st2.devices.write().unwrap();
                for dev in d.values_mut() {
                    if !dev.is_local
                        && dev.kind != crate::state::DeviceKind::Sim
                        && now.saturating_sub(dev.last_seen_ms) > 30_000
                    {
                        dev.online = false;
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }
    });
    axum::serve(l, app).await?;
    Ok(())
}

async fn admin_index() -> Response {
    admin_static(Path("index.html".into())).await
}

async fn admin_static(Path(path): Path<String>) -> Response {
    let p = if path.is_empty() {
        "index.html"
    } else {
        path.as_str()
    };
    match ADMIN.get_file(p) {
        Some(f) => {
            let mime = mime_guess::from_path(p).first_or_octet_stream();
            (
                [
                    (header::CONTENT_TYPE, mime.as_ref().to_string()),
                    (header::CACHE_CONTROL, "no-cache".into()),
                ],
                f.contents(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

fn state_json(st: &AppState) -> serde_json::Value {
    let devices: Vec<Device> = st.devices.read().unwrap().values().cloned().collect();
    let dev_json: Vec<serde_json::Value> = devices
        .iter()
        .map(|d| {
            let mut v = serde_json::to_value(d).unwrap();
            v["usable_bytes"] = serde_json::json!(d.usable_bytes());
            v
        })
        .collect();
    let offer = st.offer.read().unwrap().clone();
    serde_json::json!({
        "mesh_id": st.mesh_id(),
        "devices": dev_json,
        "models": *st.models.read().unwrap(),
        "plan": *st.plan.read().unwrap(),
        "run": *st.run.read().unwrap(),
        "downloads": *st.downloads.read().unwrap(),
        "offer": offer.as_ref().map(|o| serde_json::json!({"payload": o.qr_payload(), "svg": crate::state::qr_svg(&o.qr_payload()), "host": o.host, "port": o.control_port})),
        "policy": *st.policy.read().unwrap(),
        "models_dir": st.models_dir,
        "now_ms": crate::state::now_ms(),
        "mirrored": false,
    })
}

async fn api_state(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    if st.mirror_token.read().unwrap().is_some() {
        let snap = st.mirror.read().unwrap().clone();
        return Json(match snap {
            Some((s, _)) => s,
            None => {
                serde_json::json!({"mesh_id": "mirror", "devices": [], "models": [], "plan": null, "run": {"status": "idle", "log_tail": ["mirror: no coordinator has pushed state yet"]}, "downloads": [], "mirrored": true, "now_ms": crate::state::now_ms()})
            }
        });
    }
    Json(state_json(&st))
}

/// Coordinator → mirror push. Body: {"state": <state json>, "runs": [...]}. Bearer token required.
async fn api_relay_state(
    State(st): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Response {
    let want = st.mirror_token.read().unwrap().clone();
    let Some(want) = want else {
        return (StatusCode::NOT_FOUND, "not a mirror").into_response();
    };
    let got = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if got != want {
        return (StatusCode::UNAUTHORIZED, "bad token").into_response();
    }
    let mut s = body["state"].clone();
    s["mirrored"] = serde_json::json!(true);
    s["mirror_received_ms"] = serde_json::json!(crate::state::now_ms());
    *st.mirror.write().unwrap() = Some((s, body["runs"].clone()));
    Json(serde_json::json!({"ok": true})).into_response()
}

/// Coordinator side: push state + runs to the mirror every 2 s.
pub async fn push_loop(st: Arc<AppState>) {
    let client = reqwest::Client::new();
    loop {
        let cfg = st.push_to.read().unwrap().clone();
        if let Some((base, token)) = cfg {
            let state = state_json(&st);
            let runs = serde_json::json!(*st.runs.read().unwrap());
            let r = client
                .post(format!("{base}/api/relay/state"))
                .bearer_auth(&token)
                .json(&serde_json::json!({"state": state, "runs": runs}))
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;
            if let Err(e) = r {
                tracing::warn!("mirror push failed: {e}");
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

async fn api_catalog() -> Json<Vec<models::CatalogEntry>> {
    Json(models::catalog())
}

async fn api_rescan(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    st.scan_models();
    Json(serde_json::json!({"ok": true, "count": st.models.read().unwrap().len()}))
}

#[derive(Deserialize)]
struct DownloadReq {
    id: Option<String>,
    url: Option<String>,
    file: Option<String>,
}
async fn api_download(State(st): State<Arc<AppState>>, Json(r): Json<DownloadReq>) -> Response {
    let (url, file) = if let Some(id) = r.id {
        match models::catalog().into_iter().find(|c| c.id == id) {
            Some(c) => (c.url, c.file),
            None => return (StatusCode::NOT_FOUND, "unknown catalog id").into_response(),
        }
    } else {
        match (r.url, r.file) {
            (Some(u), Some(f)) => (u, f),
            _ => return (StatusCode::BAD_REQUEST, "need id or url+file").into_response(),
        }
    };
    match models::start_download(st, url, file).await {
        Ok(()) => Json(serde_json::json!({"ok": true})).into_response(),
        Err(e) => (StatusCode::CONFLICT, e.to_string()).into_response(),
    }
}

async fn api_model_file(State(st): State<Arc<AppState>>, Path(file): Path<String>) -> Response {
    if file.contains('/') || file.contains("..") {
        return (StatusCode::BAD_REQUEST, "bad name").into_response();
    }
    let p = st.models_dir.join(&file);
    match tokio::fs::File::open(&p).await {
        Ok(f) => {
            let len = f.metadata().await.map(|m| m.len()).unwrap_or(0);
            let stream = tokio_util_stream(f);
            (
                [
                    (header::CONTENT_TYPE, "application/octet-stream".to_string()),
                    (header::CONTENT_LENGTH, len.to_string()),
                ],
                axum::body::Body::from_stream(stream),
            )
                .into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "no such model").into_response(),
    }
}

fn tokio_util_stream(
    f: tokio::fs::File,
) -> impl futures_util::Stream<Item = Result<bytes::Bytes, std::io::Error>> {
    futures_util::stream::unfold(f, |mut f| async move {
        use tokio::io::AsyncReadExt;
        let mut buf = vec![0u8; 1 << 20];
        match f.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(bytes::Bytes::from(buf)), f))
            }
            Err(e) => Some((Err(e), f)),
        }
    })
}

async fn api_offer(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let o = st.new_offer(meshcore::CONTROL_PORT);
    Json(
        serde_json::json!({"payload": o.qr_payload(), "svg": crate::state::qr_svg(&o.qr_payload()), "host": o.host, "port": o.control_port}),
    )
}

#[derive(Deserialize)]
struct PlanReq {
    model: String,
    #[serde(default = "default_ctx")]
    n_ctx: u32,
    host: Option<String>,
}
fn default_ctx() -> u32 {
    4096
}

async fn api_plan(State(st): State<Arc<AppState>>, Json(r): Json<PlanReq>) -> Response {
    match st.make_plan(&r.model, r.n_ctx, r.host) {
        Ok(p) => {
            let args = supervisor::llama_args(&st, &p).ok();
            Json(serde_json::json!({"plan": p, "args": args})).into_response()
        }
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_run(State(st): State<Arc<AppState>>, Json(r): Json<PlanReq>) -> Response {
    let plan = match st.make_plan(&r.model, r.n_ctx, r.host) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    };
    match supervisor::start(st.clone(), plan.clone()).await {
        Ok(()) => Json(serde_json::json!({"ok": true, "plan": plan})).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn api_stop(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    supervisor::stop(&st).await;
    Json(serde_json::json!({"ok": true}))
}

async fn api_forget(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    st.devices.write().unwrap().remove(&id);
    st.pairing.lock().unwrap().forget(&id);
    Json(serde_json::json!({"ok": true}))
}

#[derive(Deserialize)]
struct LimitReq {
    usable_gb: Option<f64>,
}
async fn api_limit(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(r): Json<LimitReq>,
) -> Response {
    let mut d = st.devices.write().unwrap();
    match d.get_mut(&id) {
        Some(dev) => {
            dev.usable_override_bytes = r.usable_gb.map(|g| (g * 1e9) as u64);
            Json(serde_json::json!({"ok": true, "usable_bytes": dev.usable_bytes()}))
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "no such device").into_response(),
    }
}

#[derive(Deserialize)]
struct SimReq {
    n: usize,
    #[serde(default = "default_sim_gb")]
    usable_gb: f64,
    #[serde(default)]
    spawn: bool, // also start real local ggml-rpc-server processes so a split really runs
}
fn default_sim_gb() -> f64 {
    8.5
}
async fn api_sim_workers(State(st): State<Arc<AppState>>, Json(r): Json<SimReq>) -> Response {
    // remove old sims
    {
        let mut d = st.devices.write().unwrap();
        d.retain(|_, v| v.kind != crate::state::DeviceKind::Sim);
    }
    {
        let mut kids = st.sim_children.lock().unwrap();
        for c in kids.iter_mut() {
            let _ = c.start_kill();
        }
        kids.clear();
    }
    let mut spawned = Vec::new();
    for i in 0..r.n {
        let id = format!("sim-phone-{}", i + 1);
        let port = meshcore::RPC_PORT + 10 + i as u16;
        st.add_sim_device(&id, (r.usable_gb * 1e9) as u64, 0.3);
        if let Some(dev) = st.devices.write().unwrap().get_mut(&id) {
            dev.rpc_port = port;
            dev.name = format!("Simulated phone {}", i + 1);
        }
        if r.spawn {
            match supervisor::spawn_rpc_server(&st.llama_bin, "127.0.0.1", port, Some(2), false) {
                Ok(c) => {
                    spawned.push(port);
                    st.sim_children.lock().unwrap().push(c);
                }
                Err(e) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
                }
            }
        }
    }
    Json(serde_json::json!({"ok": true, "spawned_ports": spawned})).into_response()
}

async fn api_runs(State(st): State<Arc<AppState>>) -> Json<serde_json::Value> {
    if st.mirror_token.read().unwrap().is_some() {
        return Json(
            st.mirror
                .read()
                .unwrap()
                .as_ref()
                .map(|(_, r)| r.clone())
                .unwrap_or(serde_json::json!([])),
        );
    }
    Json(serde_json::json!(*st.runs.read().unwrap()))
}

#[derive(Deserialize)]
struct BenchReq {
    model: String,
    #[serde(default = "default_threads")]
    threads: usize,
}
fn default_threads() -> usize {
    4
}
/// Quick local llama-bench (pp128/tg32) for the laptop's bench_tps. Blocking but short.
async fn api_bench(State(st): State<Arc<AppState>>, Json(r): Json<BenchReq>) -> Response {
    let m = match st.model(&r.model) {
        Some(m) => m,
        None => return (StatusCode::NOT_FOUND, "model not found").into_response(),
    };
    let bin = st.llama_bin.join("llama-bench");
    let path = st.models_dir.join(&m.file);
    let out = tokio::process::Command::new(bin)
        .args([
            "-m",
            &path.display().to_string(),
            "-t",
            &r.threads.to_string(),
            "-p",
            "128",
            "-n",
            "32",
            "-r",
            "2",
            "-o",
            "json",
        ])
        .output()
        .await;
    match out {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout).to_string();
            let v: serde_json::Value = serde_json::from_str(&s).unwrap_or(serde_json::json!([]));
            let mut pp = 0f32;
            let mut tg = 0f32;
            for row in v.as_array().cloned().unwrap_or_default() {
                let tps = row["avg_ts"].as_f64().unwrap_or(0.0) as f32;
                if row["n_gen"].as_u64().unwrap_or(0) > 0 {
                    tg = tps;
                } else {
                    pp = tps;
                }
            }
            if let Some(d) = st.devices.write().unwrap().get_mut("local") {
                d.bench_tps = tg;
            }
            Json(serde_json::json!({"ok": o.status.success(), "prompt_tps": pp, "decode_tps": tg, "raw": v})).into_response()
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
