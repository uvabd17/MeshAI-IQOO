//! In-memory mesh state: devices, models, plan, run, downloads, timing rows, pairing persistence.

use meshcore::gguf::ModelInfo;
use meshcore::pairing::{PairedDevice, PairingBook, PairingOffer};
use meshcore::planner::{self, DeviceCap, Plan, Policy};
use meshcore::proto;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Reserve on every device until Phase 0 measures the real kill line (D016).
pub const HEADROOM_BYTES: u64 = 2_000_000_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceKind {
    Laptop,
    Phone,
    Sim,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub is_local: bool,
    pub online: bool,
    pub addr: Option<String>, // ip for remote devices
    pub rpc_port: u16,
    pub role: String, // idle | host | worker | rejected
    pub profile: Option<proto::DeviceProfile>,
    pub telemetry: Option<proto::Telemetry>,
    pub usable_override_bytes: Option<u64>,
    pub last_seen_ms: u64,
    #[serde(default)]
    pub progress: Option<Progress>,
    /// When `telemetry` was last sampled (memory figures older than the run's `ready_ms` earn no credit).
    #[serde(default)]
    pub telemetry_ms: u64,
    pub bench_tps: f32,
    /// Plan id for which this remote worker reported "RPC port accepting connections".
    #[serde(default)]
    pub worker_ready_plan: Option<String>,
    /// A plan was pushed to this device and not yet withdrawn (it may have a process running).
    #[serde(default)]
    pub has_plan: bool,
    /// Our end of this device's control socket: the address the laptop's RPC worker binds to for
    /// it, and the address it uses to reach us (H2 laptop side).
    #[serde(default)]
    pub link_local_addr: Option<String>,
    /// Identity of the live control connection; a stale handler must not touch a reconnected device.
    #[serde(default)]
    pub conn_id: u64,
    /// The last plan pushed to this device (re-sent if it reconnects during the run, L6). Not serialised.
    #[serde(skip)]
    pub last_plan: Option<proto::Plan>,
}

impl Device {
    /// One line of specs for dashboards: "SM7475 · 8 cores · 7.4 GB RAM · android/15".
    pub fn spec(&self) -> String {
        let mut parts = Vec::new();
        if let Some(p) = &self.profile {
            if !p.soc.is_empty() {
                parts.push(p.soc.split(" · ").next().unwrap_or(&p.soc).to_string());
            }
            let cores = p.cores.iter().filter(|c| c.allowed).count();
            if cores > 0 {
                parts.push(format!("{cores} cores"));
            }
            if p.total_bytes > 0 {
                parts.push(format!("{:.1} GB RAM", p.total_bytes as f64 / 1e9));
            }
            if !p.os.is_empty() {
                parts.push(p.os.split(" (").next().unwrap_or(&p.os).to_string());
            }
        }
        if let Some(o) = self.usable_override_bytes {
            parts.push(format!("capped at {:.2} GB", o as f64 / 1e9));
        }
        parts.join(" · ")
    }

    /// The single membership predicate used by cleanup, forget and failure reports (round-5 #2).
    pub fn in_run(&self) -> bool {
        self.has_plan || self.role == "host" || self.role == "worker"
    }
    pub fn usable_bytes(&self) -> u64 {
        if let Some(o) = self.usable_override_bytes {
            return o;
        }
        let avail = self.telemetry.as_ref().map(|t| t.avail_bytes).unwrap_or(0);
        avail.saturating_sub(
            self.profile
                .as_ref()
                .map(|p| p.headroom_bytes)
                .unwrap_or(HEADROOM_BYTES),
        )
    }
    pub fn supported(&self) -> bool {
        match &self.profile {
            Some(p) => p.tier != proto::Tier::Unsupported as i32,
            None => true, // sims and the laptop
        }
    }
    pub fn cap(&self) -> DeviceCap {
        let t = self.telemetry.clone().unwrap_or_default();
        DeviceCap {
            device_id: self.id.clone(),
            name: self.name.clone(),
            usable_bytes: self.usable_bytes(),
            bench_tps: self.bench_tps,
            rtt_ms_p95: t.rtt_ms_p95,
            thermal_headroom: if t.thermal_headroom.is_nan() {
                0.0
            } else {
                t.thermal_headroom
            },
            battery_pct: t.battery_pct,
            charging: t.charging,
            is_local: self.is_local,
            supported: self.supported(),
            can_host: self.kind != DeviceKind::Sim,
        }
    }
}

/// Last progress a device reported for any job (download, worker bring-up); shown on dashboards.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub job: String,
    pub fraction: f32,
    pub note: String,
    pub ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub file: String,
    pub info: ModelInfo,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RunStatus {
    pub status: String, // idle | starting | loading | ready | error | stopping
    pub host_id: Option<String>,
    pub model: Option<String>,
    pub plan_id: Option<String>,
    pub started_ms: Option<u64>,
    pub ready_ms: Option<u64>,
    pub endpoint: Option<String>,
    pub log_tail: Vec<String>,
    pub error: Option<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Download {
    pub file: String,
    pub url: String,
    pub total: u64,
    pub bytes: u64,
    pub done: bool,
    pub error: Option<String>,
    pub started_ms: u64,
}

/// One line of guidance for a device's dashboard card (T096, docs §17.4/§20): plain, short, and
/// computed cheaply in `api_state` from a snapshot of what the device last reported. `action` is
/// an optional short tag the panel can hang a button off (e.g. "measure_link"); most rules have
/// none.
#[derive(Debug, Clone, Serialize)]
pub struct Advice {
    pub device_id: String,
    pub level: &'static str, // info | warn | fail
    pub text: String,
    pub action: Option<&'static str>,
}

fn advice(
    device_id: &str,
    level: &'static str,
    text: impl Into<String>,
    action: Option<&'static str>,
) -> Advice {
    Advice {
        device_id: device_id.into(),
        level,
        text: text.into(),
        action,
    }
}

/// What a device last reported, trimmed to exactly what the advice rules need — pure and easy to
/// unit test without a live `AppState`.
#[derive(Debug, Clone, Default)]
pub struct DeviceSnapshot {
    pub device_id: String,
    pub kind: Option<DeviceKind>,
    pub is_local: bool,
    pub online: bool,
    pub addr: Option<String>,
    pub avail_bytes: u64,
    pub battery_pct: f32,
    pub charging: bool,
    pub rtt_ms_p95: f32,
    /// Only meaningful for the local device: a llama.cpp child meshd itself started is still
    /// alive while the run is idle.
    pub stray_llama: bool,
}

/// Pure advice rules (T096, docs §20) over one device's snapshot. Real phones and the laptop only
/// — simulated phones (`kind: Sim`, or `kind: None` for an unknown/offline entry) never earn
/// advice, since there is nothing on a sim the user could act on.
pub fn device_advice(d: &DeviceSnapshot) -> Vec<Advice> {
    let mut out = Vec::new();
    if d.is_local {
        if d.online && d.avail_bytes > 0 && d.avail_bytes < 4_000_000_000 {
            out.push(advice(
                &d.device_id,
                "warn",
                format!(
                    "Close heavy apps on the laptop: it has {:.1} GB available.",
                    d.avail_bytes as f64 / 1e9
                ),
                Some("close_apps"),
            ));
        }
        if d.stray_llama {
            out.push(advice(
                &d.device_id,
                "warn",
                "A llama process is still running while idle — stop it before starting a new run.",
                Some("stop_run"),
            ));
        }
        return out;
    }
    if d.kind != Some(DeviceKind::Phone) {
        return out; // sims and anything else earn no advice
    }
    if !d.online {
        out.push(advice(
            &d.device_id,
            "warn",
            "Open the MeshAI app on the phone and keep it in front.",
            Some("open_app"),
        ));
        return out; // nothing else is known about an offline phone
    }
    if !d.charging {
        out.push(advice(
            &d.device_id,
            "info",
            "Plug it in to keep it available for long runs.",
            Some("plug_in"),
        ));
    }
    if d.battery_pct > 0.0 && d.battery_pct < 30.0 {
        out.push(advice(
            &d.device_id,
            "warn",
            "Battery below 30% — plug it in soon.",
            Some("plug_in"),
        ));
    }
    if d.avail_bytes > 0 && d.avail_bytes < 1_500_000_000 {
        out.push(advice(
            &d.device_id,
            "warn",
            format!(
                "Close apps on the phone: it offers {:.1} GB right now.",
                d.avail_bytes as f64 / 1e9
            ),
            Some("close_apps"),
        ));
    }
    if d.rtt_ms_p95 > 60.0 {
        out.push(advice(
            &d.device_id,
            "warn",
            "Use a cable, USB tethering or a hotspot instead of shared Wi-Fi.",
            Some("tether"),
        ));
    }
    if d.addr.as_deref() == Some("127.0.0.1") {
        out.push(advice(
            &d.device_id,
            "warn",
            "The USB-debugging cable is slow for splits (\u{2248}1 tok/s): turn on USB tethering in the phone's Settings.",
            Some("tether"),
        ));
    }
    out
}

/// The last run intent meshd accepted (T098, docs §20): persisted so the panel can offer "Start
/// again" after a restart, even though the live run state itself does not survive one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LastRun {
    pub model: String,
    pub n_ctx: u32,
    pub host: Option<String>,
    pub ts_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRow {
    pub ts_ms: u64,
    pub model: String,
    pub mode: String,
    pub host: String,
    pub devices: usize,
    /// Streaming: client-observed time to first token. Non-streaming: llama.cpp prompt_ms.
    pub ttft_ms: u64,
    pub total_ms: u64,
    pub tokens_out: u32,
    pub prompt_tokens: u32,
    pub tps: f32,
    pub prompt_tps: f32,
    pub ok: bool,
    #[serde(default)]
    pub streaming: bool,
    /// Link kind of the run's workers: "local" (single device), "cable" (a worker reached through
    /// the adb relay at 127.0.0.1), "lan" (Wi-Fi / tethering / hotspot). Empty in rows written
    /// before 25 Sep 2026 (those splits were all over the cable).
    #[serde(default)]
    pub link: String,
}

pub struct AppState {
    pub models_dir: PathBuf,
    pub llama_bin: PathBuf,
    pub state_dir: PathBuf,
    pub lan: bool,
    pub api_token: Option<String>,
    pub devices: RwLock<BTreeMap<String, Device>>,
    pub models: RwLock<Vec<ModelEntry>>,
    pub plan: RwLock<Option<Plan>>,
    pub run: RwLock<RunStatus>,
    pub downloads: RwLock<Vec<Download>>,
    pub runs: RwLock<Vec<RunRow>>,
    pub pairing: Mutex<PairingBook>,
    pub offer: RwLock<Option<PairingOffer>>, // rendered locally only; never in state_json
    pub policy: RwLock<Policy>,
    /// pid of the host llama-server child (owned by its watcher task).
    pub host_pid: Mutex<Option<u32>>,
    pub local_worker: Mutex<Option<tokio::process::Child>>, // laptop-as-worker for a phone host
    pub local_worker_bind: Mutex<Option<String>>,
    pub sim_children: Mutex<Vec<tokio::process::Child>>, // simulated phones: live across runs
    pub plan_tx: tokio::sync::broadcast::Sender<(String, proto::Plan)>, // device_id -> plan to push
    /// (device_id, conn_id): a session whose device now belongs to another conn_id closes itself;
    /// conn_id 0 means "forgotten: send Bye and close" (round-4 #5, #8).
    pub session_ctl: tokio::sync::broadcast::Sender<(String, u64)>,
    /// Silence limit for the control plane; tests lower it (round-4 #8).
    pub silence_limit_ms: AtomicU64,
    /// Serialises the *initiation* of start/stop; the long waits run in a task tagged with `run_gen`.
    pub run_lock: tokio::sync::Mutex<()>,
    /// Held across "check generation → spawn/register/kill a process or push a plan" so a stop can
    /// never interleave with a registration (round-3 #3).
    pub proc_lock: tokio::sync::Mutex<()>,
    /// Bumped by every stop/start; background tasks quit when their generation is stale.
    pub run_gen: AtomicU64,
    pub conn_seq: AtomicU64,
    /// Mirror mode: the last (state, runs) snapshot pushed by a coordinator.
    pub mirror: RwLock<Option<(serde_json::Value, serde_json::Value)>>,
    pub mirror_token: RwLock<Option<String>>,
    pub push_to: RwLock<Option<(String, String)>>, // (base url, token)
    /// The last run intent `/api/run` accepted (T098); survives a restart via `state/last_run.json`.
    pub last_run: RwLock<Option<LastRun>>,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl AppState {
    pub fn new(
        models_dir: PathBuf,
        llama_bin: PathBuf,
        state_dir: PathBuf,
        lan: bool,
        api_token: Option<String>,
    ) -> Self {
        let (plan_tx, _) = tokio::sync::broadcast::channel(16);
        let mesh_id = format!(
            "mesh-{}",
            &hex::encode(sha2::Sha256::digest(hostname().as_bytes()))[..8]
        );
        let s = Self {
            models_dir,
            llama_bin,
            state_dir,
            lan,
            api_token,
            devices: RwLock::new(BTreeMap::new()),
            models: RwLock::new(Vec::new()),
            plan: RwLock::new(None),
            run: RwLock::new(RunStatus {
                status: "idle".into(),
                ..Default::default()
            }),
            downloads: RwLock::new(Vec::new()),
            runs: RwLock::new(Vec::new()),
            pairing: Mutex::new(PairingBook::new(mesh_id)),
            offer: RwLock::new(None),
            policy: RwLock::new(Policy::default()),
            host_pid: Mutex::new(None),
            local_worker: Mutex::new(None),
            local_worker_bind: Mutex::new(None),
            sim_children: Mutex::new(Vec::new()),
            plan_tx,
            session_ctl: tokio::sync::broadcast::channel(64).0,
            silence_limit_ms: AtomicU64::new(crate::control::SILENCE_LIMIT_MS),
            run_lock: tokio::sync::Mutex::new(()),
            proc_lock: tokio::sync::Mutex::new(()),
            run_gen: AtomicU64::new(0),
            conn_seq: AtomicU64::new(0),
            mirror: RwLock::new(None),
            mirror_token: RwLock::new(None),
            push_to: RwLock::new(None),
            last_run: RwLock::new(None),
        };
        s.load_runs();
        s.load_paired();
        s.load_last_run();
        s
    }

    pub fn mesh_id(&self) -> String {
        self.pairing.lock().unwrap().mesh_id().to_string()
    }

    pub fn gen(&self) -> u64 {
        self.run_gen.load(Ordering::SeqCst)
    }

    fn blank_device(id: &str, name: String, kind: DeviceKind, is_local: bool) -> Device {
        Device {
            id: id.into(),
            name,
            kind,
            is_local,
            online: true,
            addr: None,
            rpc_port: meshcore::RPC_PORT,
            role: "idle".into(),
            profile: None,
            telemetry: None,
            usable_override_bytes: None,
            last_seen_ms: now_ms(),
            progress: None,
            telemetry_ms: 0,
            bench_tps: 0.0,
            worker_ready_plan: None,
            has_plan: false,
            link_local_addr: None,
            conn_id: 0,
            last_plan: None,
        }
    }

    pub fn new_remote_device(id: &str, name: &str, ip: &str, rpc_port: u16) -> Device {
        let mut d = Self::blank_device(id, name.to_string(), DeviceKind::Phone, false);
        d.addr = Some(ip.to_string());
        d.rpc_port = rpc_port;
        d
    }

    // ---------- local device ----------
    pub fn refresh_local_profile(&self) {
        use sysinfo::System;
        let mut sys = System::new();
        sys.refresh_memory();
        sys.refresh_cpu_all();
        let total = sys.total_memory();
        let avail = sys.available_memory();
        let cpu = sys
            .cpus()
            .first()
            .map(|c| c.brand().to_string())
            .unwrap_or_default();
        let cores: Vec<proto::Core> = sys
            .cpus()
            .iter()
            .enumerate()
            .map(|(i, c)| proto::Core {
                index: i as u32,
                max_khz: (c.frequency() * 1000) as u32,
                capacity: 1024,
                allowed: true,
            })
            .collect();
        let profile = proto::DeviceProfile {
            device_id: "local".into(),
            os: format!(
                "{} {}",
                System::name().unwrap_or_default(),
                System::os_version().unwrap_or_default()
            ),
            soc: cpu,
            cpu_features: Vec::new(),
            cores,
            total_bytes: total,
            headroom_bytes: HEADROOM_BYTES,
            tier: proto::Tier::A as i32,
            backends: vec![],
            models: vec![],
            permissions: vec![
                format!("models folder: {}", self.models_dir.display()),
                "llama.cpp host + RPC worker: available".into(),
                if self.lan {
                    "API: open on your link, token required".into()
                } else {
                    "API: this laptop only (localhost)".into()
                },
                "control plane :7070: paired phones only".into(),
            ],
        };
        let telemetry = proto::Telemetry {
            device_id: "local".into(),
            avail_bytes: avail,
            thermal_headroom: 0.0,
            charging: true,
            battery_pct: 100.0,
            held_bytes: self.local_held_bytes(),
            ..Default::default()
        };
        let mut d = self.devices.write().unwrap();
        let e = d.entry("local".into()).or_insert_with(|| {
            Self::blank_device(
                "local",
                format!("{} (this laptop)", hostname()),
                DeviceKind::Laptop,
                true,
            )
        });
        e.profile = Some(profile);
        e.telemetry = Some(telemetry);
        e.last_seen_ms = now_ms();
        e.telemetry_ms = now_ms();
    }

    pub fn add_sim_device(&self, id: &str, usable: u64, rtt: f32) {
        let mut dev = Self::blank_device(id, id.to_string(), DeviceKind::Sim, false);
        dev.addr = Some("127.0.0.1".into());
        dev.rpc_port = 0;
        dev.telemetry = Some(proto::Telemetry {
            device_id: id.into(),
            avail_bytes: usable + HEADROOM_BYTES,
            rtt_ms_p50: rtt,
            rtt_ms_p95: rtt * 1.5,
            charging: true,
            battery_pct: 100.0,
            ..Default::default()
        });
        dev.usable_override_bytes = Some(usable);
        self.devices.write().unwrap().insert(id.into(), dev);
    }

    // ---------- models ----------
    pub fn scan_models(&self) {
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&self.models_dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) != Some("gguf") {
                    continue;
                }
                let file = p.file_name().unwrap().to_string_lossy().to_string();
                if file.starts_with("mmproj-") {
                    continue; // a vision projector is not a runnable model (paired via the catalog's `mmproj`)
                }
                if self
                    .downloads
                    .read()
                    .unwrap()
                    .iter()
                    .any(|d| d.file == file && !d.done)
                {
                    continue;
                }
                match meshcore::gguf::read(&p) {
                    // A GGUF without transformer blocks (a stable-diffusion checkpoint, a vocoder…)
                    // is not something llama-server can run; other engines will list it (D034).
                    Ok(info) if info.n_layer == 0 || !is_chat_arch(&info.arch) => {
                        tracing::info!(
                            "scan: {file} is not a chat model (arch {}), skipped — it needs its own engine (D034)",
                            info.arch
                        );
                    }
                    Ok(info) => out.push(ModelEntry {
                        file,
                        info,
                        sha256: None,
                    }),
                    Err(err) => tracing::warn!("skip {}: {err}", p.display()),
                }
            }
        }
        out.sort_by_key(|m| m.info.file_bytes);
        *self.models.write().unwrap() = out;
    }

    pub fn model(&self, file: &str) -> Option<ModelEntry> {
        self.models
            .read()
            .unwrap()
            .iter()
            .find(|m| m.file == file || m.info.name == file)
            .cloned()
    }

    // ---------- pairing ----------
    pub fn new_offer(&self, control_port: u16) -> PairingOffer {
        self.new_offer_at(None, control_port)
    }

    /// The offer names the laptop address the phone must dial; a laptop with several links
    /// (Wi-Fi + USB tethering) can pick one explicitly (`POST /api/pair/offer {"host": …}`).
    pub fn new_offer_at(&self, host: Option<String>, control_port: u16) -> PairingOffer {
        let mut hosts: Vec<String> = local_links().into_iter().map(|(ip, _)| ip).collect();
        if let Some(h) = host.filter(|h| !h.is_empty()) {
            hosts.retain(|x| x != &h);
            hosts.insert(0, h);
        }
        if hosts.is_empty() {
            hosts.push(
                local_ip_address::local_ip()
                    .map(|i| i.to_string())
                    .unwrap_or_else(|_| "127.0.0.1".into()),
            );
        }
        let o = self
            .pairing
            .lock()
            .unwrap()
            .offer_multi(hosts, control_port);
        *self.offer.write().unwrap() = Some(o.clone());
        o
    }

    fn paired_path(&self) -> PathBuf {
        self.state_dir.join("paired.json")
    }
    /// Device secrets: created 0600 from the first byte (temp file + rename), git-ignored.
    pub fn save_paired(&self) {
        let list: Vec<PairedDevice> = self.pairing.lock().unwrap().export();
        let _ = std::fs::create_dir_all(&self.state_dir);
        if let Ok(s) = serde_json::to_string_pretty(&list) {
            let p = self.paired_path();
            let tmp = p.with_extension("json.tmp");
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            if let Ok(mut f) = opts.open(&tmp) {
                use std::io::Write;
                if f.write_all(s.as_bytes()).is_ok() {
                    let _ = std::fs::rename(&tmp, &p);
                }
            }
        }
    }
    /// Paired devices survive a restart: they are listed (offline) so the admin can see and
    /// forget them, and a stale entry can never block a fresh pairing silently.
    fn load_paired(&self) {
        if let Ok(s) = std::fs::read_to_string(self.paired_path()) {
            if let Ok(list) = serde_json::from_str::<Vec<PairedDevice>>(&s) {
                {
                    let mut d = self.devices.write().unwrap();
                    for p in &list {
                        d.entry(p.device_id.clone()).or_insert_with(|| {
                            let mut dev = Self::new_remote_device(
                                &p.device_id,
                                &p.name,
                                "",
                                meshcore::RPC_PORT,
                            );
                            dev.online = false;
                            dev
                        });
                    }
                }
                self.pairing.lock().unwrap().import(list);
            }
        }
    }

    fn last_run_path(&self) -> PathBuf {
        self.state_dir.join("last_run.json")
    }
    /// Persist the run intent `/api/run` just accepted (T098) so a restarted meshd can still offer
    /// "Start again". Best-effort: a write failure is logged, never fatal to the run itself.
    pub fn save_last_run(&self, lr: &LastRun) {
        *self.last_run.write().unwrap() = Some(lr.clone());
        let _ = std::fs::create_dir_all(&self.state_dir);
        match serde_json::to_string_pretty(lr) {
            Ok(s) => {
                if let Err(e) = std::fs::write(self.last_run_path(), s) {
                    tracing::warn!("could not persist last_run.json: {e}");
                }
            }
            Err(e) => tracing::warn!("could not serialise last_run: {e}"),
        }
    }
    fn load_last_run(&self) {
        if let Ok(s) = std::fs::read_to_string(self.last_run_path()) {
            if let Ok(lr) = serde_json::from_str::<LastRun>(&s) {
                *self.last_run.write().unwrap() = Some(lr);
            }
        }
    }
    /// Is the persisted last run's model still on disk? (`run.resumable` in `/api/state`.)
    pub fn resumable(&self) -> bool {
        self.last_run
            .read()
            .unwrap()
            .as_ref()
            .is_some_and(|lr| self.model(&lr.model).is_some())
    }

    /// A llama.cpp child meshd itself started (host or local worker) is still alive while the run
    /// is idle — a crash or a missed cleanup left it behind (T096 advice rule). Cheap: at most two
    /// PID checks, and only while idle.
    pub fn stray_llama(&self) -> bool {
        if self.run.read().unwrap().status != "idle" {
            return false;
        }
        if let Some(pid) = *self.host_pid.lock().unwrap() {
            if process_alive(pid) {
                return true;
            }
        }
        self.local_worker.lock().unwrap().is_some()
    }

    /// Advice lines for every device (T096, docs §20): plugged in, close apps, the demo-critical
    /// cable warning, an offline-but-paired phone. Cheap — no I/O beyond what `/api/state` reads
    /// anyway.
    pub fn advice(&self) -> Vec<Advice> {
        let stray = self.stray_llama();
        self.devices
            .read()
            .unwrap()
            .values()
            .flat_map(|d| {
                let t = d.telemetry.as_ref();
                device_advice(&DeviceSnapshot {
                    device_id: d.id.clone(),
                    kind: Some(d.kind.clone()),
                    is_local: d.is_local,
                    online: d.online,
                    addr: d.addr.clone(),
                    avail_bytes: t.map(|t| t.avail_bytes).unwrap_or(0),
                    battery_pct: t.map(|t| t.battery_pct).unwrap_or(0.0),
                    charging: t.map(|t| t.charging).unwrap_or(true),
                    rtt_ms_p95: t.map(|t| t.rtt_ms_p95).unwrap_or(0.0),
                    stray_llama: d.is_local && stray,
                })
            })
            .collect()
    }

    // ---------- planning ----------
    pub fn make_plan(
        &self,
        model_file: &str,
        n_ctx: u32,
        prefer_host: Option<String>,
    ) -> anyhow::Result<Plan> {
        let m = self
            .model(model_file)
            .ok_or_else(|| anyhow::anyhow!("model not found: {model_file}"))?;
        let caps: Vec<DeviceCap> = self
            .devices
            .read()
            .unwrap()
            .values()
            .filter(|d| d.online)
            .map(Device::cap)
            .collect();
        let mut pol = self.policy.read().unwrap().clone();
        pol.prefer_host = prefer_host;
        pol.host_extra_bytes = self.projector_bytes(&m.file);
        let mut plan = planner::plan(&m.info, &caps, n_ctx, &pol)?;
        plan.model = m.file.clone(); // the file name is the stable id across coordinator and phones
        Ok(plan)
    }

    /// Cheap checks that must pass *before* the current run is stopped (round-4 #12) — the plan
    /// itself is made only after the stop so the planner sees the freed memory (M4, round-5 #1).
    pub fn precheck_run(
        &self,
        model_file: &str,
        n_ctx: u32,
        host: Option<&str>,
    ) -> Result<(), String> {
        let Some(m) = self.model(model_file) else {
            return Err(format!("model not found: {model_file}"));
        };
        if n_ctx == 0 {
            return Err("n_ctx must be > 0".into());
        }
        if m.info.n_ctx_train > 0 && n_ctx > m.info.n_ctx_train {
            return Err(format!(
                "n_ctx {n_ctx} exceeds the model's training context {} — pick a smaller context",
                m.info.n_ctx_train
            ));
        }
        if let Some(h) = host {
            let d = self.devices.read().unwrap();
            let Some(dev) = d.get(h) else {
                return Err(format!("unknown host device: {h}"));
            };
            if !dev.is_local && !self.lan {
                return Err("a phone can only be the host when meshd runs with --lan --api-token (it fetches the model from this API)".into());
            }
            if !dev.is_local && !dev.online {
                return Err(format!("host device {h} is offline"));
            }
        }
        Ok(())
    }

    /// Device capacities for planning a *replacement* run. A device is credited only with memory
    /// its own llama.cpp processes demonstrably hold for the live run (round-10 #1): the run must
    /// be `ready`, the device online with *measured* memory (no override) sampled after `ready_ms`,
    /// and the credit is `min(placement bytes, held_bytes)` where `held_bytes` is the anonymous RSS
    /// of the run's child processes on that device — what a stop actually returns to
    /// MemAvailable (file-backed mmap'd weights stay in page cache and were never subtracted from
    /// `avail_bytes` in the first place). The laptop measures its own children in
    /// `refresh_local_profile`; phones report theirs in `Telemetry.held_bytes`. A loading run, a
    /// stale sample, an override, an offline device or a rejected placement earn nothing.
    /// Returns (caps, credited bytes per device).
    pub fn credited_caps(&self) -> (Vec<DeviceCap>, Vec<(String, u64)>) {
        let plan = self.plan.read().unwrap().clone();
        let (ready, ready_ms) = {
            let r = self.run.read().unwrap();
            (r.status == "ready", r.ready_ms.unwrap_or(u64::MAX))
        };
        let d = self.devices.read().unwrap();
        let mut credits = Vec::new();
        let caps = d
            .values()
            .filter(|dev| dev.online)
            .map(|dev| {
                let mut cap = dev.cap();
                if !ready || dev.usable_override_bytes.is_some() || dev.telemetry_ms < ready_ms {
                    return cap;
                }
                let placement = plan.as_ref().and_then(|p| {
                    p.placements
                        .iter()
                        .find(|pl| pl.device_id == dev.id && pl.role != planner::Role::Rejected)
                });
                let Some(p) = placement else {
                    return cap;
                };
                let held = dev.telemetry.as_ref().map(|t| t.held_bytes).unwrap_or(0);
                // The run's compute buffers are anonymous memory too, so the credit may cover the
                // placement's bytes plus its reserve (never more than what is really held).
                let credit = (p.bytes + p.compute_reserve_bytes).min(held);
                if credit > 0 {
                    cap.usable_bytes = cap.usable_bytes.saturating_add(credit);
                    credits.push((dev.id.clone(), credit));
                }
                cap
            })
            .collect();
        (caps, credits)
    }

    /// Anonymous RSS (+ shmem) of our own llama.cpp children: the host llama-server and the local
    /// ggml-rpc-server, if running. Zero when nothing is running or /proc is unreadable.
    pub fn local_held_bytes(&self) -> u64 {
        let mut pids = Vec::new();
        if let Some(p) = *self.host_pid.lock().unwrap() {
            pids.push(p);
        }
        if let Some(c) = self.local_worker.lock().unwrap().as_ref() {
            if let Some(p) = c.id() {
                pids.push(p);
            }
        }
        pids.into_iter().map(rss_anon_of).sum()
    }

    /// Size of the vision projector the supervisor will pass with `--mmproj` for this model (0 for
    /// text models or when the projector file is missing). It stays in the host process
    /// (`--no-mmproj-offload`), so the planner budgets it on the host (D033, reviewer MEDIUM-3).
    pub fn projector_bytes(&self, model_file: &str) -> u64 {
        crate::models::catalog()
            .into_iter()
            .find(|c| c.file == model_file)
            .and_then(|c| c.mmproj)
            .and_then(|mm| std::fs::metadata(self.models_dir.join(mm)).ok())
            .map(|md| md.len())
            .unwrap_or(0)
    }

    /// Plan a replacement run against credited capacities (see `credited_caps`).
    pub fn make_plan_credited(
        &self,
        model_file: &str,
        n_ctx: u32,
        prefer_host: Option<String>,
    ) -> anyhow::Result<(Plan, Vec<(String, u64)>)> {
        let m = self
            .model(model_file)
            .ok_or_else(|| anyhow::anyhow!("model not found: {model_file}"))?;
        let (caps, credits) = self.credited_caps();
        let mut pol = self.policy.read().unwrap().clone();
        pol.prefer_host = prefer_host;
        pol.host_extra_bytes = self.projector_bytes(&m.file);
        let mut plan = planner::plan(&m.info, &caps, n_ctx, &pol)?;
        plan.model = m.file.clone();
        Ok((plan, credits))
    }

    pub fn set_roles_from_plan(&self, plan: &Plan) {
        let mut d = self.devices.write().unwrap();
        for dev in d.values_mut() {
            dev.role = "idle".into();
        }
        for p in &plan.placements {
            if let Some(dev) = d.get_mut(&p.device_id) {
                dev.role = match p.role {
                    planner::Role::Host => "host",
                    planner::Role::Worker => "worker",
                    planner::Role::Rejected => "rejected",
                }
                .into();
            }
        }
    }

    // ---------- runs ----------
    fn runs_path(&self) -> PathBuf {
        self.state_dir.join("runs.jsonl")
    }
    fn load_runs(&self) {
        if let Ok(s) = std::fs::read_to_string(self.runs_path()) {
            let rows: Vec<RunRow> = s
                .lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect();
            *self.runs.write().unwrap() = rows;
        }
    }
    /// Link kind of the current plan's workers (see `RunRow::link`).
    pub fn current_link_kind(&self) -> String {
        let plan = self.plan.read().unwrap();
        let devs = self.devices.read().unwrap();
        let mut kind = "local";
        if let Some(p) = plan.as_ref() {
            for pl in p
                .placements
                .iter()
                .filter(|x| x.role == planner::Role::Worker)
            {
                let addr = devs.get(&pl.device_id).and_then(|d| d.addr.clone());
                kind = if addr.as_deref() == Some("127.0.0.1") {
                    "cable"
                } else if devs
                    .get(&pl.device_id)
                    .is_some_and(|d| d.id.starts_with("sim-"))
                {
                    "loopback"
                } else {
                    "lan"
                };
                if kind == "cable" {
                    break;
                }
            }
        }
        kind.into()
    }

    pub fn record_run(&self, mut row: RunRow) {
        if row.link.is_empty() {
            row.link = self.current_link_kind();
        }
        if let Ok(line) = serde_json::to_string(&row) {
            let _ = std::fs::create_dir_all(&self.state_dir);
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(self.runs_path())
            {
                let _ = writeln!(f, "{line}");
            }
        }
        let mut r = self.runs.write().unwrap();
        r.push(row);
        if r.len() > 500 {
            let n = r.len() - 500;
            r.drain(..n);
        }
    }

    pub fn push_log(&self, line: String) {
        let mut r = self.run.write().unwrap();
        r.log_tail.push(line);
        if r.log_tail.len() > 80 {
            let n = r.log_tail.len() - 80;
            r.log_tail.drain(..n);
        }
    }

    /// The JSON the admin panel polls. `for_mirror` strips everything an internet reader must not
    /// see: addresses, raw device ids, local paths, process arguments, the host log. The pairing
    /// offer is never included in either form (C2). `mirror_snapshot_is_clean` tests this.
    pub fn state_json(&self, for_mirror: bool) -> serde_json::Value {
        let devices: Vec<Device> = self.devices.read().unwrap().values().cloned().collect();
        let dev_json: Vec<serde_json::Value> = devices
            .iter()
            .map(|d| {
                let mut v = serde_json::to_value(d).unwrap();
                v["usable_bytes"] = serde_json::json!(d.usable_bytes());
                if for_mirror {
                    v["id"] = serde_json::json!(short_id(&d.id));
                    v["name"] = serde_json::json!(match d.kind {
                        DeviceKind::Laptop => "laptop".to_string(),
                        DeviceKind::Sim => d.name.clone(),
                        DeviceKind::Phone => format!("phone {}", short_id(&d.id)),
                    });
                    v["addr"] = serde_json::Value::Null;
                    v["link_local_addr"] = serde_json::Value::Null;
                    if let Some(p) = v.get_mut("profile") {
                        if p.is_object() {
                            p["device_id"] = serde_json::json!(short_id(&d.id));
                        }
                    }
                    if let Some(t) = v.get_mut("telemetry") {
                        if t.is_object() {
                            t["device_id"] = serde_json::json!(short_id(&d.id));
                            t["cpus_allowed"] = serde_json::Value::Null;
                        }
                    }
                }
                v
            })
            .collect();
        let mut plan = serde_json::to_value(&*self.plan.read().unwrap()).unwrap();
        let mut run = serde_json::to_value(&*self.run.read().unwrap()).unwrap();
        let mut models = serde_json::to_value(&*self.models.read().unwrap()).unwrap();
        if for_mirror {
            if let Some(ps) = plan.get_mut("placements").and_then(|p| p.as_array_mut()) {
                for p in ps {
                    if let Some(id) = p["device_id"].as_str() {
                        p["device_id"] = serde_json::json!(short_id(id));
                    }
                }
            }
            if let Some(h) = plan.get("host_id").and_then(|h| h.as_str()).map(short_id) {
                plan["host_id"] = serde_json::json!(h);
            }
            run["args"] = serde_json::json!([]);
            run["endpoint"] = serde_json::Value::Null;
            run["log_tail"] = serde_json::json!([]);
            run["error"] = run["error"]
                .as_str()
                .map(|_| serde_json::json!("error (details on the coordinator)"))
                .unwrap_or(serde_json::Value::Null);
            if let Some(h) = run.get("host_id").and_then(|h| h.as_str()).map(short_id) {
                run["host_id"] = serde_json::json!(h);
            }
            if let Some(ms) = models.as_array_mut() {
                for m in ms {
                    m["info"]["path"] = serde_json::Value::Null;
                }
            }
        }
        serde_json::json!({
            "mesh_id": self.mesh_id(),
            "devices": dev_json,
            "models": models,
            "plan": plan,
            "run": run,
            "downloads": if for_mirror { serde_json::json!([]) } else { serde_json::json!(*self.downloads.read().unwrap()) },
            "policy": *self.policy.read().unwrap(),
            "models_dir": if for_mirror { serde_json::Value::Null } else { serde_json::json!(self.models_dir) },
            "now_ms": now_ms(),
            "mirrored": for_mirror,
            "lan": self.lan,
            "auth": self.api_token.is_some(),
        })
    }

    /// Timing rows for the mirror: host ids hashed.
    pub fn runs_json(&self, for_mirror: bool) -> serde_json::Value {
        let mut v = serde_json::json!(*self.runs.read().unwrap());
        if for_mirror {
            if let Some(rows) = v.as_array_mut() {
                for r in rows {
                    if let Some(h) = r["host"].as_str().map(short_id) {
                        r["host"] = serde_json::json!(h);
                    }
                }
            }
        }
        v
    }
}

/// Architectures `llama-server` can chat with. Speech-out models (`qwen3tts`, served by
/// `llama-tts`) and anything else without a chat path are left to their own engine (D034).
pub fn is_chat_arch(arch: &str) -> bool {
    !matches!(
        arch,
        "qwen3tts" | "outetts" | "wavtokenizer-dec" | "unknown"
    )
}

/// Stable, non-reversible short id for the public mirror.
pub fn short_id(id: &str) -> String {
    if id == "local" || id.starts_with("sim-") {
        return id.to_string();
    }
    format!(
        "dev-{}",
        &hex::encode(sha2::Sha256::digest(id.as_bytes()))[..6]
    )
}

/// The laptop's usable IPv4 addresses with the kind of link each belongs to, best-for-a-phone
/// first: USB tethering (RNDIS), then ethernet/hotspot, then Wi-Fi. Loopback, docker and virtual
/// bridges are skipped. Cross-platform (interface names differ; unknown names sort last).
pub fn local_links() -> Vec<(String, String)> {
    let mut out: Vec<(u8, String, String)> = Vec::new();
    if let Ok(ifs) = local_ip_address::list_afinet_netifas() {
        for (name, ip) in ifs {
            let std::net::IpAddr::V4(v4) = ip else {
                continue;
            };
            if v4.is_loopback() || v4.is_link_local() {
                continue;
            }
            let n = name.to_lowercase();
            if n.starts_with("docker")
                || n.starts_with("br-")
                || n.starts_with("veth")
                || n.starts_with("virbr")
                || n.contains("vmnet")
                || n.contains("vbox")
            {
                continue;
            }
            let (rank, kind) = if n.starts_with("enx")
                || n.contains("usb")
                || n.contains("rndis")
                || n.starts_with("ue")
            {
                (0, "usb-tether")
            } else if n.starts_with("ap") || n.contains("hotspot") {
                (1, "hotspot")
            } else if n.starts_with("en") || n.starts_with("eth") || n.contains("ethernet") {
                (2, "ethernet")
            } else if n.starts_with("wl")
                || n.contains("wi-fi")
                || n.contains("wifi")
                || n.contains("wlan")
            {
                (3, "wifi")
            } else {
                (4, "other")
            };
            out.push((rank, v4.to_string(), kind.to_string()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    out.into_iter().map(|(_, ip, k)| (ip, k)).collect()
}

pub fn hostname() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "laptop".into())
}

pub fn qr_svg(payload: &str) -> String {
    use qrcode::render::svg;
    let code = qrcode::QrCode::new(payload.as_bytes()).expect("qr");
    code.render::<svg::Color>()
        .min_dimensions(220, 220)
        .dark_color(svg::Color("currentColor"))
        .light_color(svg::Color("transparent"))
        .build()
}

pub fn qr_terminal(payload: &str) -> String {
    let code = qrcode::QrCode::new(payload.as_bytes()).expect("qr");
    code.render::<char>()
        .quiet_zone(true)
        .module_dimensions(2, 1)
        .build()
}

/// Anonymous + shmem resident bytes of a process: `/proc/<pid>/status` on Linux; elsewhere
/// (Windows, macOS) the resident set reported by sysinfo, which excludes file-backed pages on
/// Windows (private working set) — the same "what a stop gives back" meaning. 0 if unreadable.
pub fn rss_anon_of(pid: u32) -> u64 {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_to_string(format!("/proc/{pid}/status"))
            .map(|t| parse_rss_anon(&t))
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let mut s = sysinfo::System::new();
        let p = sysinfo::Pid::from_u32(pid);
        s.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[p]),
            true,
            sysinfo::ProcessRefreshKind::nothing().with_memory(),
        );
        s.process(p).map(|pr| pr.memory()).unwrap_or(0)
    }
}

/// Is the process still running? (sysinfo: works on Linux, Windows and macOS.)
pub fn process_alive(pid: u32) -> bool {
    let mut s = sysinfo::System::new();
    let p = sysinfo::Pid::from_u32(pid);
    s.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[p]),
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    s.process(p).is_some()
}

/// Ask a process to stop (`force = false`: SIGTERM on unix, TerminateProcess on Windows where no
/// gentler signal exists; `force = true`: SIGKILL / TerminateProcess).
pub fn signal_process(pid: u32, force: bool) {
    let mut s = sysinfo::System::new();
    let p = sysinfo::Pid::from_u32(pid);
    s.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[p]),
        true,
        sysinfo::ProcessRefreshKind::nothing(),
    );
    if let Some(pr) = s.process(p) {
        let sig = if force {
            sysinfo::Signal::Kill
        } else {
            sysinfo::Signal::Term
        };
        if pr.kill_with(sig).is_none() {
            pr.kill(); // platform has no such signal (Windows): terminate
        }
    }
}

/// `RssAnon` + `RssShmem` in bytes from a `/proc/<pid>/status` text; falls back to `VmRSS − RssFile`
/// on kernels without the split fields. Linux only at runtime; kept on every target for the unit test.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_rss_anon(status: &str) -> u64 {
    let kb = |key: &str| -> Option<u64> {
        status
            .lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
    };
    match (kb("RssAnon:"), kb("RssShmem:")) {
        (Some(a), sh) => (a + sh.unwrap_or(0)) * 1024,
        (None, _) => {
            let rss = kb("VmRSS:").unwrap_or(0);
            let file = kb("RssFile:").unwrap_or(0);
            rss.saturating_sub(file) * 1024
        }
    }
}

#[cfg(test)]
mod credit_tests {
    use super::*;
    use std::sync::Arc;

    fn st() -> Arc<AppState> {
        let tmp = std::env::temp_dir().join(format!(
            "meshai-credit-{}-{}",
            now_ms(),
            rand::random::<u32>()
        ));
        Arc::new(AppState::new(tmp.clone(), tmp.clone(), tmp, false, None))
    }

    fn remote(st: &AppState, id: &str, avail: u64, online: bool, override_bytes: Option<u64>) {
        let mut dev = AppState::new_remote_device(id, id, "10.0.0.9", 50052);
        dev.online = online;
        dev.usable_override_bytes = override_bytes;
        dev.telemetry = Some(proto::Telemetry {
            device_id: id.into(),
            avail_bytes: avail,
            battery_pct: 100.0,
            charging: true,
            ..Default::default()
        });
        dev.telemetry_ms = now_ms();
        st.devices.write().unwrap().insert(id.into(), dev);
    }

    fn set_held(st: &AppState, id: &str, held: u64, sampled_ms: u64) {
        let mut d = st.devices.write().unwrap();
        let dev = d.get_mut(id).unwrap();
        dev.telemetry.as_mut().unwrap().held_bytes = held;
        dev.telemetry_ms = sampled_ms;
    }

    fn placement(id: &str, bytes: u64, role: planner::Role) -> planner::Placement {
        planner::Placement {
            device_id: id.into(),
            name: id.into(),
            role,
            layer_start: 0,
            layer_end: 0,
            bytes,
            split_weight: 0.0,
            compute_reserve_bytes: 0,
            reason: String::new(),
        }
    }

    fn live_run(st: &AppState, placements: Vec<planner::Placement>, status: &str) {
        let plan = Plan {
            model: "m.gguf".into(),
            mode: planner::Mode::LayerSplit,
            n_ctx: 2048,
            host_id: "phone".into(),
            placements,
            summary: String::new(),
            total_needed_bytes: 0,
            total_usable_bytes: 0,
        };
        *st.plan.write().unwrap() = Some(plan);
        let mut r = st.run.write().unwrap();
        r.status = status.into();
        r.ready_ms = if status == "ready" {
            Some(now_ms() - 1)
        } else {
            None
        };
    }

    fn usable(st: &AppState, id: &str) -> Option<u64> {
        st.credited_caps()
            .0
            .iter()
            .find(|c| c.device_id == id)
            .map(|c| c.usable_bytes)
    }

    #[test]
    fn parse_rss_anon_reads_anon_plus_shmem_or_falls_back() {
        let t = "Name:\tllama-server\nVmRSS:\t   900000 kB\nRssAnon:\t  300000 kB\nRssFile:\t  590000 kB\nRssShmem:\t   10000 kB\n";
        assert_eq!(parse_rss_anon(t), 310_000 * 1024);
        let old = "VmRSS:\t 900000 kB\nRssFile:\t 590000 kB\n";
        assert_eq!(parse_rss_anon(old), 310_000 * 1024);
        assert_eq!(parse_rss_anon(""), 0);
        // Our own process: readable and non-zero.
        assert!(rss_anon_of(std::process::id()) > 0);
        assert_eq!(rss_anon_of(u32::MAX), 0);
    }

    #[test]
    fn a_loading_run_earns_no_credit() {
        let st = st();
        remote(&st, "phone", 3_000_000_000, true, None);
        live_run(
            &st,
            vec![placement("phone", 2_000_000_000, planner::Role::Worker)],
            "loading",
        );
        set_held(&st, "phone", 1_000_000_000, now_ms()); // the process holds memory, but the run is not ready
        assert!(st.credited_caps().1.is_empty());
    }

    #[test]
    fn credit_is_the_held_bytes_capped_at_the_placement() {
        let st = st();
        remote(&st, "phone", 3_000_000_000, true, None);
        live_run(
            &st,
            vec![placement("phone", 2_000_000_000, planner::Role::Worker)],
            "ready",
        );
        set_held(&st, "phone", 1_500_000_000, now_ms());
        let base = 3_000_000_000u64.saturating_sub(HEADROOM_BYTES);
        assert_eq!(usable(&st, "phone"), Some(base + 1_500_000_000));
        assert_eq!(
            st.credited_caps().1,
            vec![("phone".to_string(), 1_500_000_000)]
        );
        set_held(&st, "phone", 2_500_000_000, now_ms()); // more than the placement: capped
        assert_eq!(
            st.credited_caps().1,
            vec![("phone".to_string(), 2_000_000_000)]
        );
    }

    #[test]
    fn unrelated_memory_pressure_is_never_credited() {
        // avail dropped by 2 GB (another app), the run's process holds 0.3 GB: credit is 0.3 GB.
        let st = st();
        remote(&st, "phone", 3_000_000_000, true, None);
        live_run(
            &st,
            vec![placement("phone", 2_000_000_000, planner::Role::Worker)],
            "ready",
        );
        {
            let mut d = st.devices.write().unwrap();
            d.get_mut("phone")
                .unwrap()
                .telemetry
                .as_mut()
                .unwrap()
                .avail_bytes = 1_000_000_000;
        }
        set_held(&st, "phone", 300_000_000, now_ms());
        assert_eq!(
            st.credited_caps().1,
            vec![("phone".to_string(), 300_000_000)]
        );
    }

    #[test]
    fn stale_telemetry_override_offline_and_rejected_earn_nothing() {
        let st = st();
        remote(&st, "stale", 3_000_000_000, true, None);
        remote(&st, "capped", 3_000_000_000, true, Some(500_000_000));
        remote(&st, "offline", 3_000_000_000, false, None);
        remote(&st, "bystander", 3_000_000_000, true, None);
        live_run(
            &st,
            vec![
                placement("stale", 2_000_000_000, planner::Role::Worker),
                placement("capped", 400_000_000, planner::Role::Worker),
                placement("offline", 1_000_000_000, planner::Role::Worker),
                placement("bystander", 700_000_000, planner::Role::Rejected),
            ],
            "ready",
        );
        let ready_ms = st.run.read().unwrap().ready_ms.unwrap();
        set_held(&st, "stale", 1_000_000_000, ready_ms - 10); // sampled before the run was ready
        set_held(&st, "capped", 1_000_000_000, now_ms());
        set_held(&st, "bystander", 1_000_000_000, now_ms());
        assert!(
            st.credited_caps().1.is_empty(),
            "{:?}",
            st.credited_caps().1
        );
        assert_eq!(usable(&st, "capped"), Some(500_000_000));
        assert_eq!(usable(&st, "offline"), None);
    }

    #[test]
    fn no_live_plan_means_no_credit() {
        let st = st();
        remote(&st, "phone", 3_000_000_000, true, None);
        set_held(&st, "phone", 1_000_000_000, now_ms());
        assert!(st.credited_caps().1.is_empty());
    }

    /// The rule end to end through the planner: a model that does not fit the measured figures
    /// fits once the live run's held memory is credited — and still does not fit if what the run
    /// holds is too small.
    #[test]
    fn credited_plan_turns_does_not_fit_into_ok_only_when_held_memory_covers_it() {
        let st = st();
        remote(&st, "phone", HEADROOM_BYTES + 2_500_000_000, true, None);
        st.models.write().unwrap().push(ModelEntry {
            file: "m.gguf".into(),
            info: ModelInfo {
                path: "m.gguf".into(),
                file_bytes: 1_200_000_000,
                version: 3,
                arch: "qwen3".into(),
                name: "test".into(),
                quant_label: "Q8_0".into(),
                n_layer: 12,
                n_embd: 1024,
                n_head: 16,
                n_head_kv: 8,
                n_ctx_train: 32768,
                n_expert: 0,
                n_expert_used: 0,
                non_layer_bytes: 200_000_000,
                layer_bytes: vec![100_000_000; 12],
                kv_bytes_per_token: 4096,
                tensor_count: 0,
            },
            sha256: None,
        });
        live_run(
            &st,
            vec![placement("phone", 3_000_000_000, planner::Role::Host)],
            "ready",
        );
        let n_ctx = 350_000; // ~1.2 GB weights + ~1.4 GB KV ≈ 2.6 GB > 2.5 GB usable
        assert!(
            st.make_plan("m.gguf", n_ctx, None).is_err(),
            "must not fit without credit"
        );
        set_held(&st, "phone", 3_000_000_000, now_ms());
        let (plan, credits) = st
            .make_plan_credited("m.gguf", n_ctx, None)
            .expect("fits once the held 3 GB is credited");
        assert_eq!(plan.mode, planner::Mode::Single);
        assert_eq!(credits, vec![("phone".to_string(), 3_000_000_000)]);
        set_held(&st, "phone", 50_000_000, now_ms());
        assert!(
            st.make_plan_credited("m.gguf", n_ctx, None).is_err(),
            "a 0.05 GB credit must not make it fit"
        );
    }
}

#[cfg(test)]
mod advice_tests {
    use super::*;

    fn phone() -> DeviceSnapshot {
        DeviceSnapshot {
            device_id: "phone".into(),
            kind: Some(DeviceKind::Phone),
            is_local: false,
            online: true,
            addr: Some("192.168.1.7".into()),
            avail_bytes: 3_000_000_000,
            battery_pct: 80.0,
            charging: true,
            rtt_ms_p95: 5.0,
            stray_llama: false,
        }
    }

    #[test]
    fn a_healthy_phone_earns_no_advice() {
        assert!(device_advice(&phone()).is_empty());
    }

    #[test]
    fn offline_phone_gets_one_line_and_nothing_else() {
        let mut d = phone();
        d.online = false;
        let a = device_advice(&d);
        assert_eq!(a.len(), 1);
        assert!(a[0].text.contains("Open the MeshAI app"));
    }

    #[test]
    fn not_charging_and_low_battery_both_fire() {
        let mut d = phone();
        d.charging = false;
        d.battery_pct = 25.0;
        let a = device_advice(&d);
        assert_eq!(a.len(), 2);
        assert!(a.iter().any(|x| x.text.contains("Plug it in")));
        assert!(a.iter().any(|x| x.text.contains("Battery below 30%")));
    }

    #[test]
    fn low_memory_names_the_gb_offered() {
        let mut d = phone();
        d.avail_bytes = 800_000_000;
        let a = device_advice(&d);
        assert_eq!(a.len(), 1);
        assert!(a[0].text.contains("0.8 GB"), "{}", a[0].text);
        assert_eq!(a[0].level, "warn");
    }

    #[test]
    fn high_rtt_suggests_a_cable_or_tethering() {
        let mut d = phone();
        d.rtt_ms_p95 = 90.0;
        let a = device_advice(&d);
        assert_eq!(a.len(), 1);
        assert!(a[0].text.contains("cable, USB tethering or a hotspot"));
    }

    #[test]
    fn adb_relay_address_warns_about_the_cable_speed() {
        let mut d = phone();
        d.addr = Some("127.0.0.1".into());
        let a = device_advice(&d);
        assert_eq!(a.len(), 1);
        assert!(a[0].text.contains("USB tethering"));
    }

    #[test]
    fn sim_devices_earn_no_advice() {
        let mut d = phone();
        d.kind = Some(DeviceKind::Sim);
        d.charging = false;
        d.avail_bytes = 100_000_000;
        assert!(device_advice(&d).is_empty());
    }

    #[test]
    fn laptop_low_memory_and_stray_process_both_fire() {
        let d = DeviceSnapshot {
            device_id: "local".into(),
            kind: Some(DeviceKind::Laptop),
            is_local: true,
            online: true,
            addr: None,
            avail_bytes: 2_000_000_000,
            battery_pct: 100.0,
            charging: true,
            rtt_ms_p95: 0.0,
            stray_llama: true,
        };
        let a = device_advice(&d);
        assert_eq!(a.len(), 2);
        assert!(a.iter().any(|x| x.text.contains("Close heavy apps")));
        assert!(a.iter().any(|x| x.text.contains("llama process")));
    }

    #[test]
    fn healthy_laptop_earns_no_advice() {
        let d = DeviceSnapshot {
            device_id: "local".into(),
            kind: Some(DeviceKind::Laptop),
            is_local: true,
            online: true,
            addr: None,
            avail_bytes: 12_000_000_000,
            battery_pct: 100.0,
            charging: true,
            rtt_ms_p95: 0.0,
            stray_llama: false,
        };
        assert!(device_advice(&d).is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirror_snapshot_is_clean() {
        let tmp = std::env::temp_dir().join(format!("meshai-test-{}", now_ms()));
        let st = AppState::new(tmp.clone(), tmp.clone(), tmp.clone(), false, None);
        // A phone with a raw ANDROID_ID, LAN address and link address; a run log full of addresses.
        let mut d =
            AppState::new_remote_device("aa09341810e79e0c", "POCO F5", "192.168.1.77", 50052);
        d.link_local_addr = Some("192.168.1.14".into());
        d.profile = Some(proto::DeviceProfile {
            device_id: "aa09341810e79e0c".into(),
            ..Default::default()
        });
        d.telemetry = Some(proto::Telemetry {
            device_id: "aa09341810e79e0c".into(),
            cpus_allowed: "0-7".into(),
            ..Default::default()
        });
        st.devices.write().unwrap().insert(d.id.clone(), d);
        st.push_log("host: llama-server -m /mnt/storage/x.gguf --rpc 192.168.1.77:50052".into());
        {
            let mut r = st.run.write().unwrap();
            r.host_id = Some("aa09341810e79e0c".into());
            r.error = Some("worker 192.168.1.77:50052 not reachable".into());
            r.args = vec!["/mnt/storage/x.gguf".into()];
        }
        st.record_run(RunRow {
            ts_ms: 1,
            model: "m".into(),
            mode: "LayerSplit".into(),
            host: "aa09341810e79e0c".into(),
            devices: 2,
            ttft_ms: 1,
            total_ms: 2,
            tokens_out: 3,
            prompt_tokens: 4,
            tps: 1.0,
            prompt_tps: 1.0,
            ok: true,
            streaming: true,
            link: String::new(),
        });
        let s = st.state_json(true).to_string() + &st.runs_json(true).to_string();
        let ipv4 = regex_lite_find_ipv4(&s);
        assert!(ipv4.is_none(), "mirror snapshot leaks an address: {ipv4:?}");
        assert!(
            !s.contains("aa09341810e79e0c"),
            "mirror snapshot leaks the raw device id"
        );
        assert!(
            !s.contains("POCO F5"),
            "mirror snapshot leaks the phone display name"
        );
        assert!(
            !s.contains("/mnt/storage"),
            "mirror snapshot leaks a local path"
        );
        assert!(
            !s.contains("llama-server"),
            "mirror snapshot leaks the host log"
        );
        assert!(s.contains("dev-"), "hashed ids expected");
        // The local form still has what the laptop admin needs.
        let local = st.state_json(false).to_string();
        assert!(local.contains("192.168.1.77"));
        let _ = std::fs::remove_dir_all(tmp);
    }

    /// Tiny IPv4 finder (no regex crate): four dot-separated numeric groups.
    fn regex_lite_find_ipv4(s: &str) -> Option<String> {
        let b = s.as_bytes();
        let mut i = 0;
        while i < b.len() {
            if b[i].is_ascii_digit() {
                let start = i;
                let mut groups = 0;
                let mut j = i;
                loop {
                    let g = j;
                    while j < b.len() && b[j].is_ascii_digit() && j - g < 3 {
                        j += 1;
                    }
                    if j == g {
                        break;
                    }
                    groups += 1;
                    if groups == 4 {
                        let cand = &s[start..j];
                        if cand.split('.').all(|p| p.parse::<u8>().is_ok()) {
                            return Some(cand.to_string());
                        }
                        break;
                    }
                    if j < b.len() && b[j] == b'.' {
                        j += 1;
                    } else {
                        break;
                    }
                }
                i = j.max(start + 1);
            } else {
                i += 1;
            }
        }
        None
    }
}
