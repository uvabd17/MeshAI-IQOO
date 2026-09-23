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
    /// Serialises the *initiation* of start/stop; the long waits run in a task tagged with `run_gen`.
    pub run_lock: tokio::sync::Mutex<()>,
    /// Bumped by every stop/start; background tasks quit when their generation is stale.
    pub run_gen: AtomicU64,
    pub conn_seq: AtomicU64,
    /// Mirror mode: the last (state, runs) snapshot pushed by a coordinator.
    pub mirror: RwLock<Option<(serde_json::Value, serde_json::Value)>>,
    pub mirror_token: RwLock<Option<String>>,
    pub push_to: RwLock<Option<(String, String)>>, // (base url, token)
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
            run_lock: tokio::sync::Mutex::new(()),
            run_gen: AtomicU64::new(0),
            conn_seq: AtomicU64::new(0),
            mirror: RwLock::new(None),
            mirror_token: RwLock::new(None),
            push_to: RwLock::new(None),
        };
        s.load_runs();
        s.load_paired();
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
            bench_tps: 0.0,
            worker_ready_plan: None,
            has_plan: false,
            link_local_addr: None,
            conn_id: 0,
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
        let ip = local_ip_address::local_ip()
            .map(|i| i.to_string())
            .unwrap_or_else(|_| "127.0.0.1".into());
        let o = self.pairing.lock().unwrap().offer(&ip, control_port);
        *self.offer.write().unwrap() = Some(o.clone());
        o
    }

    fn paired_path(&self) -> PathBuf {
        self.state_dir.join("paired.json")
    }
    /// Device secrets: owner-only file (0600), git-ignored.
    pub fn save_paired(&self) {
        let list: Vec<PairedDevice> = self.pairing.lock().unwrap().export();
        let _ = std::fs::create_dir_all(&self.state_dir);
        if let Ok(s) = serde_json::to_string_pretty(&list) {
            let p = self.paired_path();
            let _ = std::fs::write(&p, s);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
            }
        }
    }
    fn load_paired(&self) {
        if let Ok(s) = std::fs::read_to_string(self.paired_path()) {
            if let Ok(list) = serde_json::from_str::<Vec<PairedDevice>>(&s) {
                self.pairing.lock().unwrap().import(list);
            }
        }
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
        });
        let s = st.state_json(true).to_string() + &st.runs_json(true).to_string();
        let ipv4 = regex_lite_find_ipv4(&s);
        assert!(ipv4.is_none(), "mirror snapshot leaks an address: {ipv4:?}");
        assert!(
            !s.contains("aa09341810e79e0c"),
            "mirror snapshot leaks the raw device id"
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
