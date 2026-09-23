//! JSON API for the admin panel + static admin files + /v1 proxy.
//!
//! Exposure (C1): binds to 127.0.0.1 unless `--lan`; with `--lan` an API token is mandatory and
//! every API route except the admin files and model downloads requires it (`x-mesh-token`
//! header or `Authorization: Bearer`). No CORS layer. Cross-site defences even without a token:
//! non-GET requests must carry a JSON content type (a browser form cannot), and on localhost the
//! Host header must be a loopback name (DNS rebinding).

use crate::models;
use crate::state::AppState;
use crate::supervisor;
use axum::extract::{Path, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use include_dir::{include_dir, Dir};
use serde::Deserialize;
use std::sync::Arc;

static ADMIN: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../admin/src");

/// Full coordinator router.
pub fn router(st: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(|| async { Redirect::temporary("/admin/") }))
        .route("/admin", get(|| async { Redirect::temporary("/admin/") }))
        .route("/admin/", get(admin_index))
        .route("/admin/{*path}", get(admin_static))
        .route("/api/state", get(api_state))
        .route("/api/catalog", get(api_catalog))
        .route("/api/runs", get(api_runs))
        .route("/api/models/file/{file}", get(api_model_file))
        .route("/api/models/rescan", post(api_rescan))
        .route("/api/models/download", post(api_download))
        .route("/api/pair/offer", post(api_offer))
        .route("/api/plan", post(api_plan))
        .route("/api/run", post(api_run))
        .route("/api/stop", post(api_stop))
        .route("/api/devices/{id}", delete(api_forget))
        .route("/api/devices/{id}/limit", post(api_limit))
        .route("/api/sim/workers", post(api_sim_workers))
        .route("/api/bench", post(api_bench))
        .route("/v1/{*rest}", axum::routing::any(crate::proxy::v1))
        .layer(middleware::from_fn_with_state(st.clone(), guard))
        .with_state(st)
}

/// Mirror router (C3): read-only admin + the relay ingress. Nothing else exists on the cloud box.
pub fn mirror_router(st: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(|| async { Redirect::temporary("/admin/") }))
        .route("/admin", get(|| async { Redirect::temporary("/admin/") }))
        .route("/admin/", get(admin_index))
        .route("/admin/{*path}", get(admin_static))
        .route("/api/state", get(api_state))
        .route("/api/catalog", get(api_catalog))
        .route("/api/runs", get(api_runs))
        .route("/api/relay/state", post(api_relay_state))
        .with_state(st)
}

pub async fn serve(st: Arc<AppState>, port: u16, mirror: bool) -> anyhow::Result<()> {
    let bind = if st.lan || mirror {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    let app = if mirror {
        mirror_router(st.clone())
    } else {
        router(st.clone())
    };
    let l = tokio::net::TcpListener::bind((bind, port)).await?;
    tracing::info!(
        "api/admin bound to {bind}:{port}{}",
        if st.api_token.is_some() {
            " (token required)"
        } else {
            ""
        }
    );
    if !mirror {
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
    }
    axum::serve(l, app).await?;
    Ok(())
}

fn presented_token(req: &Request) -> Option<String> {
    if let Some(v) = req
        .headers()
        .get("x-mesh-token")
        .and_then(|v| v.to_str().ok())
    {
        return Some(v.to_string());
    }
    req.headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

fn host_is_loopback(req: &Request) -> bool {
    let h = req
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let h = h.rsplit_once(':').map(|(a, _)| a).unwrap_or(h);
    h == "localhost" || h == "127.0.0.1" || h == "[::1]" || h == "::1"
}

/// Request guard: token (when configured), JSON-only mutations, loopback Host on localhost.
async fn guard(State(st): State<Arc<AppState>>, req: Request, next: Next) -> Response {
    let path = req.uri().path().to_string();
    let is_get =
        req.method() == axum::http::Method::GET || req.method() == axum::http::Method::HEAD;
    let is_static = path.starts_with("/admin") || path == "/";
    let is_model_file = path.starts_with("/api/models/file/");
    // DNS-rebinding defence when we only listen on loopback.
    if !st.lan && !host_is_loopback(&req) {
        return (StatusCode::FORBIDDEN, "host header is not loopback").into_response();
    }
    // A browser form cannot send application/json: block cross-site form POSTs even with no token.
    if !is_get && !is_static {
        let ct = req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !ct.starts_with("application/json") {
            return (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "content-type must be application/json",
            )
                .into_response();
        }
    }
    let Some(want) = st.api_token.as_deref() else {
        return next.run(req).await;
    };
    // Token configured: everything but the admin files, the catalog and model downloads needs it
    // (with --lan even GET /api/state is sensitive: addresses, args, logs).
    let open = is_static || is_model_file || (is_get && path == "/api/catalog");
    if open {
        return next.run(req).await;
    }
    let ok = presented_token(&req)
        .map(|t| meshcore::pairing::ct_eq(t.as_bytes(), want.as_bytes()))
        .unwrap_or(false);
    if !ok {
        return (StatusCode::UNAUTHORIZED, "x-mesh-token required").into_response();
    }
    next.run(req).await
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
    Json(st.state_json(false))
}

/// Coordinator → mirror push. Body: {"state": <stripped state>, "runs": [...]}. Bearer token required.
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
    if !meshcore::pairing::ct_eq(got.as_bytes(), want.as_bytes()) {
        return (StatusCode::UNAUTHORIZED, "bad token").into_response();
    }
    if body.to_string().len() > 2 << 20 {
        return (StatusCode::PAYLOAD_TOO_LARGE, "snapshot too large").into_response();
    }
    let mut s = body["state"].clone();
    s["mirrored"] = serde_json::json!(true);
    s["mirror_received_ms"] = serde_json::json!(crate::state::now_ms());
    *st.mirror.write().unwrap() = Some((s, body["runs"].clone()));
    Json(serde_json::json!({"ok": true})).into_response()
}

/// Coordinator side: push a stripped snapshot + hashed runs to the mirror every 2 s (D014).
pub async fn push_loop(st: Arc<AppState>) {
    let client = reqwest::Client::new();
    loop {
        let cfg = st.push_to.read().unwrap().clone();
        if let Some((base, token)) = cfg {
            let state = st.state_json(true);
            let runs = st.runs_json(true);
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
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
    }
}

/// Serves a catalog file to a phone host; supports a single `Range: bytes=N-` for resume (M7).
async fn api_model_file(
    State(st): State<Arc<AppState>>,
    Path(file): Path<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    if !models::valid_model_name(&file) {
        return (StatusCode::BAD_REQUEST, "bad name").into_response();
    }
    let p = st.models_dir.join(&file);
    let Ok(mut f) = tokio::fs::File::open(&p).await else {
        return (StatusCode::NOT_FOUND, "no such model").into_response();
    };
    let len = f.metadata().await.map(|m| m.len()).unwrap_or(0);
    let start: u64 = headers
        .get(header::RANGE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("bytes="))
        .and_then(|v| v.split('-').next())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if start >= len && len > 0 {
        // `bytes */len` lets a client whose .part is already complete finish instead of restarting.
        return (
            StatusCode::RANGE_NOT_SATISFIABLE,
            [(header::CONTENT_RANGE, format!("bytes */{len}"))],
            "",
        )
            .into_response();
    }
    use tokio::io::AsyncSeekExt;
    if start > 0 && f.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "seek").into_response();
    }
    let stream = file_stream(f);
    let mut resp = (
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CONTENT_LENGTH, (len - start).to_string()),
            (header::ACCEPT_RANGES, "bytes".to_string()),
        ],
        axum::body::Body::from_stream(stream),
    )
        .into_response();
    if start > 0 {
        *resp.status_mut() = StatusCode::PARTIAL_CONTENT;
        resp.headers_mut().insert(
            header::CONTENT_RANGE,
            format!("bytes {start}-{}/{len}", len.saturating_sub(1))
                .parse()
                .unwrap(),
        );
    }
    resp
}

fn file_stream(
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

/// The pairing offer is returned only here, to the (authenticated, local) admin that will render
/// the QR. It is never part of `/api/state` (C2).
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

/// Stops the previous run, plans against the freed memory (M4), records the new plan and returns
/// while bring-up continues in the background (N5). Initiation is serialised (M10).
async fn api_run(State(st): State<Arc<AppState>>, Json(r): Json<PlanReq>) -> Response {
    let _g = st.run_lock.lock().await;
    // Validate before touching the current run: a refused request must not stop it (round-4 #12).
    st.refresh_local_profile();
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
    // A phone host fetches the model from this API, so it needs --lan (round-3 #2).
    let host_is_remote = st
        .devices
        .read()
        .unwrap()
        .get(&plan.host_id)
        .map(|d| !d.is_local)
        .unwrap_or(false);
    if host_is_remote && !st.lan {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error": "a phone can only be the host when meshd runs with --lan --api-token (it fetches the model from this API)"})),
        )
            .into_response();
    }
    supervisor::stop(&st).await;
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
    let _g = st.run_lock.lock().await;
    supervisor::stop(&st).await;
    Json(serde_json::json!({"ok": true}))
}

async fn api_forget(
    State(st): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    supervisor::forget_device(&st, &id).await;
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
    if r.n > 4 {
        return (StatusCode::BAD_REQUEST, "at most 4 simulated phones").into_response();
    }
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
    Json(st.runs_json(false))
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
/// Quick local llama-bench (pp128/tg32) for the laptop's bench_tps.
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
            &r.threads.min(64).to_string(),
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
