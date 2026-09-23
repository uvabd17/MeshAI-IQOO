//! In-memory mesh state: devices, models, plan, run, downloads, timing rows.

use meshcore::gguf::ModelInfo;
use meshcore::pairing::{PairingBook, PairingOffer};
use meshcore::planner::{self, DeviceCap, Plan, Policy};
use meshcore::proto;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

pub const HEADROOM_BYTES: u64 = 2_000_000_000; // 2 GB reserve on every device until Phase 0 measures the real kill line

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
    pub bench_tps: f32,
}

impl Device {
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
        }
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRow {
    pub ts_ms: u64,
    pub model: String,
    pub mode: String,
    pub host: String,
    pub devices: usize,
    pub ttft_ms: u64,
    pub total_ms: u64,
    pub tokens_out: u32,
    pub prompt_tokens: u32,
    pub tps: f32,
    pub prompt_tps: f32,
    pub ok: bool,
}

pub struct AppState {
    pub models_dir: PathBuf,
    pub llama_bin: PathBuf,
    pub state_dir: PathBuf,
    pub devices: RwLock<BTreeMap<String, Device>>,
    pub models: RwLock<Vec<ModelEntry>>,
    pub plan: RwLock<Option<Plan>>,
    pub run: RwLock<RunStatus>,
    pub downloads: RwLock<Vec<Download>>,
    pub runs: RwLock<Vec<RunRow>>,
    pub pairing: Mutex<PairingBook>,
    pub offer: RwLock<Option<PairingOffer>>,
    pub policy: RwLock<Policy>,
    pub child: Mutex<Option<tokio::process::Child>>,
    pub sim_children: Mutex<Vec<tokio::process::Child>>, // simulated phones: live across runs
    pub local_worker: Mutex<Option<tokio::process::Child>>, // laptop-as-worker for a phone host: per run
    pub plan_tx: tokio::sync::broadcast::Sender<(String, proto::Plan)>, // device_id -> plan to push
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl AppState {
    pub fn new(models_dir: PathBuf, llama_bin: PathBuf, state_dir: PathBuf) -> Self {
        let (plan_tx, _) = tokio::sync::broadcast::channel(16);
        let mesh_id = format!(
            "mesh-{}",
            &hex::encode(sha2::Sha256::digest(hostname().as_bytes()))[..8]
        );
        let s = Self {
            models_dir,
            llama_bin,
            state_dir,
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
            child: Mutex::new(None),
            sim_children: Mutex::new(Vec::new()),
            local_worker: Mutex::new(None),
            plan_tx,
        };
        s.load_runs();
        s
    }

    pub fn mesh_id(&self) -> String {
        self.pairing.lock().unwrap().mesh_id().to_string()
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
        };
        let telemetry = proto::Telemetry {
            device_id: "local".into(),
            avail_bytes: avail,
            thermal_headroom: 0.0,
            charging: true,
            battery_pct: 100.0,
            ..Default::default()
        };
        let mut d = self.devices.write().unwrap();
        let e = d.entry("local".into()).or_insert_with(|| Device {
            id: "local".into(),
            name: format!("{} (this laptop)", hostname()),
            kind: DeviceKind::Laptop,
            is_local: true,
            online: true,
            addr: None,
            rpc_port: meshcore::RPC_PORT,
            role: "idle".into(),
            profile: None,
            telemetry: None,
            usable_override_bytes: None,
            last_seen_ms: now_ms(),
            bench_tps: 0.0,
        });
        e.profile = Some(profile);
        e.telemetry = Some(telemetry);
        e.last_seen_ms = now_ms();
    }

    pub fn add_sim_device(&self, id: &str, usable: u64, rtt: f32) {
        let mut d = self.devices.write().unwrap();
        d.insert(
            id.into(),
            Device {
                id: id.into(),
                name: id.into(),
                kind: DeviceKind::Sim,
                is_local: false,
                online: true,
                addr: Some("127.0.0.1".into()),
                rpc_port: 0,
                role: "idle".into(),
                profile: None,
                telemetry: Some(proto::Telemetry {
                    device_id: id.into(),
                    avail_bytes: usable + HEADROOM_BYTES,
                    rtt_ms_p50: rtt,
                    rtt_ms_p95: rtt * 1.5,
                    charging: true,
                    battery_pct: 100.0,
                    ..Default::default()
                }),
                usable_override_bytes: Some(usable),
                last_seen_ms: now_ms(),
                bench_tps: 0.0,
            },
        );
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
                // skip files still being downloaded
                let file = p.file_name().unwrap().to_string_lossy().to_string();
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
                    Ok(info) => out.push(ModelEntry {
                        file,
                        info,
                        sha256: None,
                    }),
                    Err(err) => tracing::warn!("skip {}: {err}", p.display()),
                }
            }
        }
        out.sort_by(|a, b| a.info.file_bytes.cmp(&b.info.file_bytes));
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
        let ip = local_ip_address::local_ip()
            .map(|i| i.to_string())
            .unwrap_or_else(|_| "127.0.0.1".into());
        let o = self.pairing.lock().unwrap().offer(&ip, control_port);
        *self.offer.write().unwrap() = Some(o.clone());
        o
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
        let mut plan = planner::plan(&m.info, &caps, n_ctx, &pol)?;
        plan.model = m.file.clone(); // the file name is the stable id across coordinator and phones
        Ok(plan)
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
    pub fn record_run(&self, row: RunRow) {
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
        if r.log_tail.len() > 60 {
            let n = r.log_tail.len() - 60;
            r.log_tail.drain(..n);
        }
    }
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

use sha2::Digest;
