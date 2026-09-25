//! The Calculate step (docs/MESHAI.md §17.4, D032, task T074): the unchanged planner's plan, the
//! model's layer config, and the measured run history → which device holds which layers, what
//! each stack costs per token, a prediction for one request, and plain-words verdicts.
//!
//! Every number is tagged (`cost::Src`). Speeds come only from this laptop's own `runs.jsonl`
//! rows and the phones' bench reports; when nothing was measured the prediction is `Unknown` and
//! the panel shows no number.

use crate::state::{AppState, RunRow};
use meshcore::cost::{self, DevicePerf, LinkInput, Prediction, Request, Src, StackInput};
use meshcore::gguf::{self, ModelInfo};
use meshcore::layers::{self, LayerConfig, UnitKind};
use meshcore::planner::{self, DeviceCap, Mode, Plan, Policy, Role, WORKER_COMPUTE_RESERVE_BYTES};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_ctx() -> u32 {
    4096
}
fn default_prompt() -> u32 {
    500
}
fn default_answer() -> u32 {
    200
}

#[derive(Debug, Clone, Deserialize)]
pub struct CalcReq {
    pub model: String,
    #[serde(default = "default_ctx")]
    pub n_ctx: u32,
    pub host: Option<String>,
    #[serde(default = "default_prompt")]
    pub prompt_tokens: u32,
    #[serde(default = "default_answer")]
    pub answer_tokens: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceVerdict {
    pub device_id: String,
    pub name: String,
    pub code: &'static str,
    pub text: String,
    /// How many blocks this device could hold on its own at this context (0 = none).
    pub max_layers: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct History {
    pub tok_s: f32,
    pub ttft_ms: u64,
    pub prompt_tokens: u32,
    pub when_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Alternative {
    pub label: String,
    pub prediction: Prediction,
    pub why_not: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CalcResponse {
    pub ok: bool,
    pub model: String,
    pub n_ctx: u32,
    pub plan: Option<Plan>,
    pub error: Option<String>,
    pub fix_hints: Vec<String>,
    pub credited: Vec<(String, u64)>,
    pub n_layer: u32,
    pub block_bytes_avg: u64,
    pub host_fixed_bytes: u64,
    pub activation_bytes: u64,
    pub prediction: Option<Prediction>,
    pub history: Option<History>,
    pub devices: Vec<DeviceVerdict>,
    pub verdict_code: &'static str,
    pub verdict: String,
    pub speed_label: &'static str,
    pub alternatives: Vec<Alternative>,
    pub warnings: Vec<String>,
}

/// Layer configs are read from the GGUF header (fast) but cached per call so a device's speed
/// derived from another model does not re-read that file many times.
struct Cfgs<'a> {
    st: &'a AppState,
    map: HashMap<String, Option<LayerConfig>>,
}

impl<'a> Cfgs<'a> {
    fn get(&mut self, file: &str) -> Option<&LayerConfig> {
        if !self.map.contains_key(file) {
            let p = self.st.models_dir.join(file);
            let c = gguf::read_full(&p).ok().map(|g| layers::layer_config(&g));
            self.map.insert(file.to_string(), c);
        }
        self.map.get(file).and_then(|c| c.as_ref())
    }
}

/// Totals of the units that always sit on the host (embedding, head, other).
fn host_fixed(cfg: &LayerConfig) -> (u64, u64, u64) {
    let mut w = (0u64, 0u64, 0u64);
    for u in cfg.units.iter().filter(|u| u.kind != UnitKind::Block) {
        w.0 += u.bytes;
        w.1 += u.read_bytes_per_token;
        w.2 += u.flops_per_token;
    }
    w
}

/// One device's whole-model stack (used to back out speeds from single-device runs).
fn whole_stack(cfg: &LayerConfig, id: &str) -> StackInput {
    let (b, r, f, kv) = cfg.stack(0, cfg.shape.n_layer);
    let (hb, hr, hf) = host_fixed(cfg);
    StackInput {
        device_id: id.into(),
        is_host: true,
        layer_start: 0,
        layer_end: cfg.shape.n_layer,
        weight_bytes: b + hb,
        read_bytes_per_token: r + hr,
        flops_per_token: f + hf,
        kv_bytes_per_token: kv,
    }
}

fn stacks_from(plan: &Plan, cfg: &LayerConfig) -> Vec<StackInput> {
    plan.placements
        .iter()
        .filter(|p| p.role != Role::Rejected)
        .map(|p| {
            let (b, r, f, kv) = cfg.stack(p.layer_start, p.layer_end);
            let (hb, hr, hf) = if p.role == Role::Host {
                host_fixed(cfg)
            } else {
                (0, 0, 0)
            };
            StackInput {
                device_id: p.device_id.clone(),
                is_host: p.role == Role::Host,
                layer_start: p.layer_start,
                layer_end: p.layer_end,
                weight_bytes: b + hb,
                read_bytes_per_token: r + hr,
                flops_per_token: f + hf,
                kv_bytes_per_token: kv,
            }
        })
        .collect()
}

/// Effective decode bandwidth and prefill compute of a device, backed out of its latest good
/// single-device run (any model). Phones without a hosted run fall back to their bench report
/// on the reference model (decode only; prefill speed then estimated from the decode figure).
fn device_perf(
    st: &AppState,
    cfgs: &mut Cfgs,
    id: &str,
    bench_tps: f32,
    same_model: &str,
    warnings: &mut Vec<String>,
) -> Option<DevicePerf> {
    // Effective speeds are lower bounds: a tiny model is overhead-bound and under-reads memory,
    // so the best figure seen across the device's recent single-device runs (any model) is the
    // one to trust. Up to 12 rows are considered.
    let runs = st.runs.read().unwrap();
    let rows: Vec<RunRow> = runs
        .iter()
        .rev()
        .filter(|r| r.ok && r.mode == "Single" && r.host == id && r.tokens_out >= 8 && r.tps > 0.0)
        .take(12)
        .cloned()
        .collect();
    drop(runs);
    // The same model's own run is the truest figure (same bytes-per-token regime); otherwise the
    // best figure across other models.
    let rows: Vec<RunRow> = if rows.iter().any(|r| r.model == same_model) {
        rows.into_iter().filter(|r| r.model == same_model).collect()
    } else {
        rows
    };
    let mut best_bw: Option<f64> = None;
    let mut best_g: Option<f64> = None;
    let mut best_model = String::new();
    for r in &rows {
        let Some(cfg) = cfgs.get(&r.model) else {
            continue;
        };
        let s = whole_stack(cfg, id);
        let pos = r.prompt_tokens as u64 + r.tokens_out as u64 / 2;
        let bw = cost::bandwidth_from_run(
            s.read_bytes_per_token,
            pos * s.kv_bytes_per_token,
            r.tps as f64,
        );
        if best_bw.is_none_or(|b| bw > b) {
            best_bw = Some(bw);
            best_model = r.model.clone();
        }
        if r.prompt_tps > 0.0 && r.prompt_tokens >= 16 {
            let g = cost::gflops_from_run(s.flops_per_token, r.prompt_tps as f64);
            if best_g.is_none_or(|b| g > b) {
                best_g = Some(g);
            }
        }
    }
    if let Some(bw) = best_bw {
        let (g, gsrc) = match best_g {
            Some(g) => (g, Src::Derived),
            None => {
                warnings.push(format!(
                    "{id}: prefill speed estimated from decode (no prompt of ≥16 tokens measured yet)"
                ));
                (bw / 1e9 * 8.5, Src::Estimate)
            }
        };
        warnings.push(format!(
            "{id}: decode bandwidth {:.1} GB/s from its best single-device run ({best_model})",
            bw / 1e9
        ));
        return Some(DevicePerf {
            device_id: id.into(),
            bw_bytes_per_s: bw,
            bw_src: Src::Derived,
            gflops: g,
            gflops_src: gsrc,
        });
    }
    if bench_tps > 0.0 {
        if let Some(cfg) = cfgs.get(crate::models::BENCH_MODEL_FILE) {
            let s = whole_stack(cfg, id);
            let bw = cost::bandwidth_from_run(
                s.read_bytes_per_token,
                32 * s.kv_bytes_per_token,
                bench_tps as f64,
            );
            warnings.push(format!(
                "{id}: speed from its bench on {} (decode only; prefill estimated)",
                crate::models::BENCH_MODEL_FILE
            ));
            return Some(DevicePerf {
                device_id: id.into(),
                bw_bytes_per_s: bw,
                bw_src: Src::Derived,
                gflops: bw / 1e9 * 8.5,
                gflops_src: Src::Estimate,
            });
        }
    }
    None
}

/// Fixed per-token link cost for a worker. The USB-debugging cable (adb relay, addr 127.0.0.1)
/// is derived from this laptop's measured split runs; other links use the measured RTT as an
/// estimate until a probe exists (T075).
fn link_input(
    st: &AppState,
    cfgs: &mut Cfgs,
    laptop_bw: Option<f64>,
    id: &str,
    addr: Option<&str>,
    rtt_p95_ms: f32,
    warnings: &mut Vec<String>,
) -> Option<LinkInput> {
    // Measured split rows over the same kind of link: the leftover after the (slightly smaller
    // than laptop-alone) compute is the per-token link cost. Rows written before 25 Sep 2026
    // carry no link kind; they were all over the cable.
    let kind = if addr == Some("127.0.0.1") {
        "cable"
    } else {
        "lan"
    };
    {
        let runs = st.runs.read().unwrap();
        let rows: Vec<RunRow> = runs
            .iter()
            .filter(|r| {
                r.ok && r.mode == "LayerSplit"
                    && r.devices >= 2
                    && r.tokens_out >= 8
                    && r.tps > 0.0
                    && (r.link == kind || (kind == "cable" && r.link.is_empty()))
            })
            .cloned()
            .collect();
        drop(runs);
        let mut alphas = Vec::new();
        for r in rows.iter().rev().take(6) {
            let (Some(cfg), Some(bw)) = (cfgs.get(&r.model), laptop_bw) else {
                continue;
            };
            let s = whole_stack(cfg, "local");
            let pos = r.prompt_tokens as u64 + r.tokens_out as u64 / 2;
            let full_ms =
                (s.read_bytes_per_token + pos * s.kv_bytes_per_token) as f64 / bw * 1000.0;
            if let Some(a) = cost::alpha_from_split(1000.0 / r.tps as f64, 0.93 * full_ms) {
                alphas.push(a);
            }
        }
        if !alphas.is_empty() {
            alphas.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let med = alphas[alphas.len() / 2];
            warnings.push(format!(
                "{id}: link cost {med:.0} ms per token from {} measured split(s) over the {kind}",
                alphas.len()
            ));
            return Some(LinkInput {
                device_id: id.into(),
                alpha_ms: med,
                alpha_src: Src::Derived,
                beta_bytes_per_s: if kind == "cable" { 10e6 } else { 50e6 },
                beta_src: Src::Estimate,
            });
        }
    }
    if addr == Some("127.0.0.1") {
        warnings.push(format!(
            "{id}: no measured split over the cable yet; link cost unknown"
        ));
        return None;
    }
    if rtt_p95_ms > 0.0 {
        warnings.push(format!(
            "{id}: link cost estimated from RTT p95 {rtt_p95_ms:.0} ms (no probe yet)"
        ));
        return Some(LinkInput {
            device_id: id.into(),
            alpha_ms: rtt_p95_ms as f64 * 1.5,
            alpha_src: Src::Estimate,
            beta_bytes_per_s: 50e6,
            beta_src: Src::Estimate,
        });
    }
    None
}

/// One row of the device table at calculation time.
struct DevRow {
    id: String,
    name: String,
    is_local: bool,
    online: bool,
    addr: Option<String>,
    usable: u64,
    bench: f32,
    rtt: f32,
}

/// Every known device, online or not (a `calculate` verdict wants to say why an offline device
/// isn't in the plan too). Shared by `calculate` and `feasibility` (T096) so the two never read
/// the device table differently.
fn device_rows(st: &AppState) -> Vec<DevRow> {
    st.devices
        .read()
        .unwrap()
        .values()
        .map(|d| DevRow {
            id: d.id.clone(),
            name: d.name.clone(),
            is_local: d.is_local,
            online: d.online,
            addr: d.addr.clone(),
            usable: d.usable_bytes(),
            bench: d.bench_tps,
            rtt: d.telemetry.as_ref().map(|t| t.rtt_ms_p95).unwrap_or(0.0),
        })
        .collect()
}

/// Effective decode/prefill speed of every online device and the per-link cost of every online
/// worker, backed out of `model_file`'s own measured history (see `device_perf`/`link_input`).
/// Shared by `calculate` (one model) and `feasibility` (every catalog model, T096) so a tok/s
/// figure is never computed two different ways.
fn speeds_for(
    st: &AppState,
    cfgs: &mut Cfgs,
    devices: &[DevRow],
    model_file: &str,
    warnings: &mut Vec<String>,
) -> (Vec<DevicePerf>, Vec<LinkInput>) {
    let mut perf: Vec<DevicePerf> = Vec::new();
    let mut links: Vec<LinkInput> = Vec::new();
    let laptop_bw = devices
        .iter()
        .find(|d| d.is_local)
        .and_then(|d| device_perf(st, cfgs, &d.id, d.bench, model_file, warnings))
        .map(|p| {
            let bw = p.bw_bytes_per_s;
            perf.push(p);
            bw
        });
    for d in devices.iter().filter(|d| !d.is_local && d.online) {
        if let Some(p) = device_perf(st, cfgs, &d.id, d.bench, model_file, warnings) {
            perf.push(p);
        }
        if let Some(l) = link_input(
            st,
            cfgs,
            laptop_bw,
            &d.id,
            d.addr.as_deref(),
            d.rtt,
            warnings,
        ) {
            links.push(l);
        }
    }
    (perf, links)
}

fn speed_label(tok_s: &cost::Est) -> &'static str {
    if !tok_s.is_known() {
        "unknown"
    } else if tok_s.v >= 10.0 {
        "fast"
    } else if tok_s.v >= 3.0 {
        "ok"
    } else {
        "slow"
    }
}

fn fix_hints(err: &str) -> Vec<String> {
    let mut h = Vec::new();
    let e = err.to_lowercase();
    if e.contains("bytes") || e.contains("hold") || e.contains("pool") || e.contains("memory") {
        h.push("Lower the conversation memory to 2k.".into());
        h.push("Pick a smaller model or a smaller quantisation.".into());
        h.push("Close apps on the phone and on the laptop to free memory.".into());
        h.push("Pair another device.".into());
    }
    if e.contains("rtt") {
        h.push("Use a cable, USB tethering or a hotspot instead of shared Wi-Fi.".into());
    }
    if e.contains("battery") {
        h.push("Plug the phone in.".into());
    }
    if h.is_empty() {
        h.push("Open the Details drawer for the raw reason.".into());
    }
    h
}

pub fn calculate(st: &AppState, req: &CalcReq) -> CalcResponse {
    let mut warnings = Vec::new();
    let mut cfgs = Cfgs {
        st,
        map: HashMap::new(),
    };
    let Some(cfg) = cfgs.get(&req.model).cloned() else {
        return CalcResponse {
            ok: false,
            model: req.model.clone(),
            n_ctx: req.n_ctx,
            plan: None,
            error: Some(format!("model {} is not on disk", req.model)),
            fix_hints: vec!["Download the model first (Details → model files).".into()],
            credited: vec![],
            n_layer: 0,
            block_bytes_avg: 0,
            host_fixed_bytes: 0,
            activation_bytes: 0,
            prediction: None,
            history: None,
            devices: vec![],
            verdict_code: "cannot_run",
            verdict: format!("Cannot run: {} is not on disk", req.model),
            speed_label: "unknown",
            alternatives: vec![],
            warnings,
        };
    };
    let n_layer = cfg.shape.n_layer;
    let (blk_bytes, _, _, kv_all) = cfg.stack(0, n_layer);
    let block_avg = if n_layer > 0 {
        blk_bytes / n_layer as u64
    } else {
        0
    };
    let kv_per_layer_ctx = if n_layer > 0 {
        kv_all / n_layer as u64 * req.n_ctx as u64
    } else {
        0
    };
    let host_fixed_bytes = cfg.host_fixed_bytes();

    // Devices: verdicts and what each could hold on its own.
    let devices: Vec<DevRow> = device_rows(st);
    let per_layer = block_avg + kv_per_layer_ctx;
    let max_layers = |usable: u64| -> u32 {
        if per_layer == 0 {
            return 0;
        }
        (usable.saturating_sub(WORKER_COMPUTE_RESERVE_BYTES) / per_layer).min(n_layer as u64) as u32
    };

    let planned = st.make_plan_credited(&req.model, req.n_ctx, req.host.clone());
    let (plan, credited, error) = match planned {
        Ok((p, c)) => (Some(p), c, None),
        Err(e) => (None, vec![], Some(e.to_string())),
    };

    // Speeds.
    let (perf, links) = speeds_for(st, &mut cfgs, &devices, &req.model, &mut warnings);

    let request = Request {
        prompt_tokens: req.prompt_tokens,
        answer_tokens: req.answer_tokens,
        activation_bytes: cfg.activation_bytes.max(4),
        ubatch: 512,
    };

    let mut verdicts: Vec<DeviceVerdict> = Vec::new();
    let mut prediction = None;
    let mut alternatives = Vec::new();
    let (verdict_code, verdict, label): (&'static str, String, &'static str);

    match &plan {
        Some(p) => {
            let stacks = stacks_from(p, &cfg);
            let pred = cost::predict(&request, &stacks, &perf, &links);
            for pl in &p.placements {
                let usable = devices
                    .iter()
                    .find(|d| d.id == pl.device_id)
                    .map(|d| d.usable)
                    .unwrap_or(0);
                let ml = max_layers(usable);
                let (code, text) = match pl.role {
                    Role::Host => (
                        "used",
                        format!(
                            "host: layers {}–{} plus the input/output tables",
                            pl.layer_start,
                            pl.layer_end.saturating_sub(1)
                        ),
                    ),
                    Role::Worker => (
                        "used",
                        format!(
                            "helper: layers {}–{} ({} layers)",
                            pl.layer_start,
                            pl.layer_end.saturating_sub(1),
                            pl.layer_end - pl.layer_start
                        ),
                    ),
                    Role::Rejected => {
                        if pl.reason.starts_with("not needed") {
                            (
                                "capable_not_needed",
                                if ml > 0 {
                                    format!("could hold up to {ml} of {n_layer} layers; not needed")
                                } else {
                                    "not needed; could not hold even one layer right now".into()
                                },
                            )
                        } else {
                            ("not_capable", pl.reason.clone())
                        }
                    }
                };
                verdicts.push(DeviceVerdict {
                    device_id: pl.device_id.clone(),
                    name: pl.name.clone(),
                    code,
                    text,
                    max_layers: ml,
                });
            }
            label = speed_label(&pred.tok_s);
            match p.mode {
                Mode::Single => {
                    let host = p.placements.iter().find(|x| x.role == Role::Host);
                    verdict_code = "can_run_single";
                    verdict = format!(
                        "Can run on {} alone.",
                        host.map(|h| h.name.clone())
                            .unwrap_or_else(|| p.host_id.clone())
                    );
                    // The split nobody should run, costed anyway so the screen can say why not.
                    if let Some(w) = p
                        .placements
                        .iter()
                        .find(|x| x.role == Role::Rejected && x.reason.starts_with("not needed"))
                    {
                        let usable = devices
                            .iter()
                            .find(|d| d.id == w.device_id)
                            .map(|d| d.usable)
                            .unwrap_or(0);
                        let k = max_layers(usable);
                        if k > 0 && k < n_layer {
                            let cut = n_layer - k;
                            let alt_stacks = vec![
                                StackInput {
                                    device_id: p.host_id.clone(),
                                    is_host: true,
                                    layer_start: 0,
                                    layer_end: cut,
                                    weight_bytes: cfg.stack(0, cut).0 + host_fixed_bytes,
                                    read_bytes_per_token: cfg.stack(0, cut).1 + host_fixed(&cfg).1,
                                    flops_per_token: cfg.stack(0, cut).2 + host_fixed(&cfg).2,
                                    kv_bytes_per_token: cfg.stack(0, cut).3,
                                },
                                StackInput {
                                    device_id: w.device_id.clone(),
                                    is_host: false,
                                    layer_start: cut,
                                    layer_end: n_layer,
                                    weight_bytes: cfg.stack(cut, n_layer).0,
                                    read_bytes_per_token: cfg.stack(cut, n_layer).1,
                                    flops_per_token: cfg.stack(cut, n_layer).2,
                                    kv_bytes_per_token: cfg.stack(cut, n_layer).3,
                                },
                            ];
                            let ap = cost::predict(&request, &alt_stacks, &perf, &links);
                            let why = if ap.tok_s.is_known() && pred.tok_s.is_known() {
                                if ap.tok_s.v < pred.tok_s.v * 0.9 {
                                    format!(
                                        "about {:.1}× slower: {}",
                                        pred.tok_s.v / ap.tok_s.v.max(0.01),
                                        ap.bottleneck
                                    )
                                } else {
                                    "no faster than the laptop alone; splitting is for memory, not speed".into()
                                }
                            } else {
                                "its speed is unknown until a split has been measured".into()
                            };
                            alternatives.push(Alternative {
                                label: format!(
                                    "{} 0–{} + {} {}–{} (not chosen)",
                                    p.placements
                                        .iter()
                                        .find(|x| x.role == Role::Host)
                                        .map(|h| h.name.as_str())
                                        .unwrap_or("host"),
                                    cut.saturating_sub(1),
                                    w.name,
                                    cut,
                                    n_layer.saturating_sub(1)
                                ),
                                prediction: ap,
                                why_not: why,
                            });
                        }
                    }
                }
                Mode::LayerSplit => {
                    let n = p
                        .placements
                        .iter()
                        .filter(|x| x.role != Role::Rejected)
                        .count();
                    verdict_code = "can_run_split";
                    verdict = format!("Can run split across {n} devices.");
                }
            }
            prediction = Some(pred);
        }
        None => {
            verdict_code = "cannot_run";
            verdict = format!("Cannot run: {}", error.clone().unwrap_or_default());
            label = "unknown";
            for d in &devices {
                let ml = max_layers(d.usable);
                verdicts.push(DeviceVerdict {
                    device_id: d.id.clone(),
                    name: d.name.clone(),
                    code: if ml > 0 { "capable" } else { "not_capable" },
                    text: if ml > 0 {
                        format!("could hold up to {ml} of {n_layer} layers")
                    } else {
                        "cannot hold even one layer right now".into()
                    },
                    max_layers: ml,
                });
            }
        }
    }

    let history = {
        let mode = plan.as_ref().map(|p| match p.mode {
            Mode::Single => "Single",
            Mode::LayerSplit => "LayerSplit",
        });
        let runs = st.runs.read().unwrap();
        runs.iter()
            .rev()
            .find(|r| {
                r.ok && r.model == req.model
                    && r.tokens_out >= 8
                    && mode.is_none_or(|m| r.mode == m)
            })
            .map(|r| History {
                tok_s: r.tps,
                ttft_ms: r.ttft_ms,
                prompt_tokens: r.prompt_tokens,
                when_ms: r.ts_ms,
            })
    };

    let hints = error.as_deref().map(fix_hints).unwrap_or_default();
    CalcResponse {
        ok: plan.is_some(),
        model: req.model.clone(),
        n_ctx: req.n_ctx,
        plan,
        error,
        fix_hints: hints,
        credited,
        n_layer,
        block_bytes_avg: block_avg,
        host_fixed_bytes,
        activation_bytes: cfg.activation_bytes,
        prediction,
        history,
        devices: verdicts,
        verdict_code,
        verdict,
        speed_label: label,
        alternatives,
        warnings,
    }
}

// ---------------------------------------------------------------------------------------------
// Feasibility (T096, docs §20): every catalog model + every runnable file already on disk, sorted
// into easy / hard-but-doable / impossible against the mesh's current (credited) capacities — the
// same capacities `/api/plan` and `/api/run` use.

#[derive(Debug, Clone, Deserialize)]
pub struct FeasReq {
    #[serde(default = "default_ctx")]
    pub n_ctx: u32,
}

/// One concrete change that would flip a model's category (or explain a "hard"/"impossible" one).
#[derive(Debug, Clone, Serialize)]
pub struct Need {
    pub what: &'static str, // "context" | "free_memory" | "device" | "quant" | "download"
    pub detail: String,
    pub device_id: Option<String>,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EstTokS {
    pub v: f64,
    pub src: Src,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeasModel {
    pub id: String,
    pub file: String,
    pub name: String,
    pub on_disk: bool,
    pub category: &'static str, // "easy" | "hard" | "impossible"
    pub reason: String,
    pub needs: Vec<Need>,
    pub plan_mode: Option<Mode>,
    pub devices_used: u32,
    pub est_tok_s: Option<EstTokS>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeasResponse {
    pub models: Vec<FeasModel>,
}

/// A model built from the catalog's own `bytes`/`layers` for a file that is not on disk yet: no
/// tensor table exists to read, so weights are assumed even across layers and the KV cache is
/// left at 0 (unknown until the real shapes are known) — always labelled an estimate by the
/// caller. `layers` must be > 0 (the caller special-cases companion files like a vision projector).
fn synth_model_info(file: &str, name: &str, bytes: u64, layers: u32) -> ModelInfo {
    let per_layer = bytes / layers.max(1) as u64;
    ModelInfo {
        path: file.into(),
        file_bytes: bytes,
        version: 3,
        arch: "unknown".into(),
        name: name.into(),
        quant_label: "unknown".into(),
        n_layer: layers,
        n_embd: 0,
        n_head: 0,
        n_head_kv: 0,
        n_ctx_train: 0,
        n_expert: 0,
        n_expert_used: 0,
        non_layer_bytes: 0,
        layer_bytes: vec![per_layer; layers as usize],
        kv_bytes_per_token: 0,
        tensor_count: 0,
    }
}

/// What it would take to run `info` on ONE device instead of splitting (the `hard: runs only as a
/// split` case): the gap between the best host-capable device's usable memory and what a
/// single-device run needs. `None` when no device qualifies at all (shouldn't happen — a split
/// was already chosen, meaning some devices are eligible) or the gap has already closed.
fn single_device_gap(
    caps: &[DeviceCap],
    info: &ModelInfo,
    host_extra: u64,
    n_ctx: u32,
) -> Option<Need> {
    let needed = info.weight_bytes()
        + info.kv_bytes(n_ctx)
        + host_extra
        + planner::HOST_COMPUTE_RESERVE_BYTES;
    let best = caps
        .iter()
        .filter(|d| d.can_host && d.supported)
        .max_by_key(|d| d.usable_bytes)?;
    if best.usable_bytes >= needed {
        return None;
    }
    let deficit = needed - best.usable_bytes;
    Some(Need {
        what: "free_memory",
        detail: format!(
            "free about {:.2} GB more on {} to run without splitting ({:.2} GB needed, {:.2} GB usable)",
            deficit as f64 / 1e9,
            best.name,
            needed as f64 / 1e9,
            best.usable_bytes as f64 / 1e9
        ),
        device_id: Some(best.device_id.clone()),
        bytes: Some(deficit),
    })
}

/// Turn a planner refusal into the "impossible" reason text + (usually) one concrete `Need`.
fn impossible_reason(err: &planner::PlanError, ctx: u32) -> (String, Option<Need>) {
    use planner::PlanError::*;
    match err {
        NoDevices => ("no eligible device is online right now".to_string(), None),
        NoHost => (
            "no eligible device can host llama-server — only compute-only workers are online"
                .to_string(),
            Some(Need {
                what: "device",
                detail:
                    "pair or bring online a device that can act as host (a phone or this laptop)"
                        .into(),
                device_id: None,
                bytes: None,
            }),
        ),
        HostTooSmall {
            host,
            usable,
            fixed,
        } => {
            let deficit = fixed.saturating_sub(*usable);
            (
                format!(
                    "host {host} holds {:.2} GB but the embeddings/output tensors + compute reserve alone need {:.2} GB at {ctx} tokens of context — short {:.2} GB",
                    *usable as f64 / 1e9,
                    *fixed as f64 / 1e9,
                    deficit as f64 / 1e9
                ),
                Some(Need {
                    what: "free_memory",
                    detail: format!("free at least {:.0} MB on {host}", deficit as f64 / 1e6),
                    device_id: Some(host.clone()),
                    bytes: Some(deficit),
                }),
            )
        }
        DoesNotFit { needed, available } => {
            let deficit = needed.saturating_sub(*available);
            (
                format!(
                    "needs {:.2} GB pooled (weights + KV + compute reserves at {ctx} tokens of context) but the online devices pool only {:.2} GB — short {:.2} GB",
                    *needed as f64 / 1e9,
                    *available as f64 / 1e9,
                    deficit as f64 / 1e9
                ),
                Some(Need {
                    what: "device",
                    detail: format!(
                        "pair one more device with at least {:.2} GB free, or free that much across the paired devices",
                        deficit as f64 / 1e9
                    ),
                    device_id: None,
                    bytes: Some(deficit),
                }),
            )
        }
        HostNotEligible { host, reason } => (
            format!("requested host {host} is not eligible: {reason}"),
            None,
        ),
    }
}

struct Decision {
    category: &'static str,
    reason: String,
    plan: Option<Plan>,
    devices_used: u32,
    needs: Vec<Need>,
}

/// Pure category rule (T096): given the mesh's device capacities and one model's shape, decide
/// easy / hard / impossible at `n_ctx`, falling back to a 2,048-token check for "hard", and to the
/// planner's own refusal for "impossible". No I/O — everything comes from `caps`/`info`/`pol`.
fn decide(
    caps: &[DeviceCap],
    info: &ModelInfo,
    pol: &Policy,
    n_ctx: u32,
    estimate_note: Option<&str>,
) -> Decision {
    let suffix = |s: &str| match estimate_note {
        Some(n) => format!("{s} — {n}"),
        None => s.to_string(),
    };
    match planner::plan(info, caps, n_ctx, pol) {
        Ok(p) if p.mode == Mode::Single => Decision {
            category: "easy",
            reason: suffix(&p.summary),
            plan: Some(p),
            devices_used: 1,
            needs: vec![],
        },
        Ok(p) => {
            let used = p
                .placements
                .iter()
                .filter(|x| x.role != Role::Rejected)
                .count() as u32;
            let mut needs = Vec::new();
            if let Some(nd) = single_device_gap(caps, info, pol.host_extra_bytes, n_ctx) {
                needs.push(nd);
            }
            Decision {
                category: "hard",
                reason: suffix(&format!(
                    "runs only when split across {used} devices: {}",
                    p.summary
                )),
                devices_used: used,
                plan: Some(p),
                needs,
            }
        }
        Err(_) => {
            let plan_2k = (n_ctx > 2048)
                .then(|| planner::plan(info, caps, 2048, pol).ok())
                .flatten();
            if let Some(p2) = plan_2k {
                let used = p2
                    .placements
                    .iter()
                    .filter(|x| x.role != Role::Rejected)
                    .count() as u32;
                Decision {
                    category: "hard",
                    reason: suffix(&format!(
                        "does not fit at {n_ctx} tokens of context; fits at 2,048 tokens across {used} device{}",
                        if used == 1 { "" } else { "s" }
                    )),
                    devices_used: used,
                    plan: Some(p2),
                    needs: vec![Need {
                        what: "context",
                        detail: format!("use a 2,048-token conversation memory instead of {n_ctx}"),
                        device_id: None,
                        bytes: None,
                    }],
                }
            } else {
                let floor_ctx = n_ctx.min(2048);
                let err = planner::plan(info, caps, floor_ctx, pol).unwrap_err();
                let (reason, need) = impossible_reason(&err, floor_ctx);
                Decision {
                    category: "impossible",
                    reason: suffix(&reason),
                    devices_used: 0,
                    plan: None,
                    needs: need.into_iter().collect(),
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn feasibility_one(
    st: &AppState,
    cfgs: &mut Cfgs,
    devices: &[DevRow],
    caps: &[DeviceCap],
    id: &str,
    file: &str,
    name: &str,
    cat_bytes: u64,
    cat_layers: u32,
    n_ctx: u32,
) -> FeasModel {
    let on_disk = st.model(file).is_some();
    if !on_disk && cat_layers == 0 {
        // A file the catalog lists that has no transformer blocks of its own (the vision
        // projector): it can never run standalone.
        return FeasModel {
            id: id.into(),
            file: file.into(),
            name: name.into(),
            on_disk,
            category: "impossible",
            reason: "not a standalone model: it is a companion file (a vision projector) loaded alongside another model, never run by itself".into(),
            needs: vec![],
            plan_mode: None,
            devices_used: 0,
            est_tok_s: None,
        };
    }
    let info = match st.model(file) {
        Some(m) => m.info,
        None => synth_model_info(file, name, cat_bytes, cat_layers),
    };
    let estimate_note = (!on_disk).then_some(
        "estimated from the catalog's file size and layer count — not downloaded, so weights are assumed even across layers and the KV cache is not modelled (the real need will be higher once it is on disk)",
    );
    let host_extra = if on_disk { st.projector_bytes(file) } else { 0 };
    let mut pol = st.policy.read().unwrap().clone();
    pol.host_extra_bytes = host_extra;
    pol.prefer_host = None;

    let mut d = decide(caps, &info, &pol, n_ctx, estimate_note);
    if !on_disk {
        d.needs.insert(
            0,
            Need {
                what: "download",
                detail: format!("download {file} ({:.2} GB)", cat_bytes as f64 / 1e9),
                device_id: None,
                bytes: Some(cat_bytes),
            },
        );
    }

    let est_tok_s = if on_disk {
        match &d.plan {
            Some(p) => match cfgs.get(file).cloned() {
                Some(cfg) => {
                    let mut warnings = Vec::new();
                    let (perf, links) = speeds_for(st, cfgs, devices, file, &mut warnings);
                    let stacks = stacks_from(p, &cfg);
                    let request = Request {
                        prompt_tokens: default_prompt(),
                        answer_tokens: default_answer(),
                        activation_bytes: cfg.activation_bytes.max(4),
                        ubatch: 512,
                    };
                    let pred = cost::predict(&request, &stacks, &perf, &links);
                    pred.tok_s.is_known().then_some(EstTokS {
                        v: pred.tok_s.v,
                        src: pred.tok_s.src,
                    })
                }
                None => None,
            },
            None => None,
        }
    } else {
        None
    };

    FeasModel {
        id: id.into(),
        file: file.into(),
        name: name.into(),
        on_disk,
        category: d.category,
        reason: d.reason,
        needs: d.needs,
        plan_mode: d.plan.as_ref().map(|p| p.mode.clone()),
        devices_used: d.devices_used,
        est_tok_s,
    }
}

/// Every catalog entry (whether downloaded or not) plus any runnable file already on disk but not
/// in the catalog (T096, docs §20). Categories are computed only from the mesh's current credited
/// device capacities (the same the Run step uses) and the GGUF layer bytes.
pub fn feasibility(st: &AppState, req: &FeasReq) -> FeasResponse {
    let mut cfgs = Cfgs {
        st,
        map: HashMap::new(),
    };
    let devices = device_rows(st);
    let (caps, _credits) = st.credited_caps();

    let catalog = crate::models::catalog();
    let mut entries: Vec<(String, String, String, u64, u32)> = catalog
        .iter()
        .map(|c| {
            (
                c.id.clone(),
                c.file.clone(),
                c.name.clone(),
                c.bytes,
                c.layers,
            )
        })
        .collect();
    for m in st.models.read().unwrap().iter() {
        if catalog.iter().any(|c| c.file == m.file) {
            continue;
        }
        // The GGUF's own `general.name` is sometimes blank; fall back to the file name so the
        // panel never shows an empty card title.
        let name = if m.info.name.trim().is_empty() {
            m.file.clone()
        } else {
            m.info.name.clone()
        };
        entries.push((
            m.file.clone(),
            m.file.clone(),
            name,
            m.info.file_bytes,
            m.info.n_layer,
        ));
    }

    let models = entries
        .into_iter()
        .map(|(id, file, name, bytes, layers)| {
            feasibility_one(
                st, &mut cfgs, &devices, &caps, &id, &file, &name, bytes, layers, req.n_ctx,
            )
        })
        .collect();
    FeasResponse { models }
}

#[cfg(test)]
mod feasibility_tests {
    use super::*;

    fn model(n_layer: u32, layer_mb: u64, non_layer_mb: u64) -> ModelInfo {
        ModelInfo {
            path: "x".into(),
            file_bytes: 0,
            version: 3,
            arch: "test".into(),
            name: "test-model".into(),
            quant_label: "Q4".into(),
            n_layer,
            n_embd: 2048,
            n_head: 16,
            n_head_kv: 4,
            n_ctx_train: 32768,
            n_expert: 0,
            n_expert_used: 0,
            non_layer_bytes: non_layer_mb * 1_000_000,
            layer_bytes: vec![layer_mb * 1_000_000; n_layer as usize],
            kv_bytes_per_token: 48 * 2 * 4 * 128 * 2, // ~96 KB, all layers
            tensor_count: 0,
        }
    }
    fn dev(id: &str, gb: f64, local: bool) -> DeviceCap {
        DeviceCap {
            device_id: id.into(),
            name: id.into(),
            usable_bytes: (gb * 1e9) as u64,
            bench_tps: 0.0,
            rtt_ms_p95: 1.0,
            thermal_headroom: 0.4,
            battery_pct: 80.0,
            charging: true,
            is_local: local,
            supported: true,
            can_host: true,
        }
    }

    #[test]
    fn fits_one_device_is_easy() {
        let m = model(28, 20, 300); // ~0.86 GB
        let d = decide(
            &[dev("local", 12.0, true)],
            &m,
            &Policy::default(),
            4096,
            None,
        );
        assert_eq!(d.category, "easy");
        assert_eq!(d.devices_used, 1);
        assert!(d.needs.is_empty());
        assert_eq!(d.plan.unwrap().mode, Mode::Single);
    }

    #[test]
    fn fits_only_split_is_hard_with_a_free_memory_need() {
        let m = model(48, 380, 500); // ~18.7 GB
        let d = decide(
            &[dev("local", 12.0, true), dev("phoneA", 8.5, false)],
            &m,
            &Policy::default(),
            8192,
            None,
        );
        assert_eq!(d.category, "hard");
        assert_eq!(d.devices_used, 2);
        assert!(d.reason.contains("split across 2 devices"), "{}", d.reason);
        assert_eq!(d.needs.len(), 1);
        assert_eq!(d.needs[0].what, "free_memory");
        assert!(d.needs[0].device_id.is_some());
    }

    #[test]
    fn fits_only_at_2k_is_hard_with_a_context_need() {
        // 12 layers × 10 MB + 50 MB head + 300 MB host reserve ≈ 470 MB fixed; the KV cache
        // (48 × 2 × 4 × 128 × 2 ≈ 96 KB/token, all layers) adds ≈ 805 MB at 8192 tokens of context
        // (needed ≈ 1.28 GB, over a 1.0 GB device) but only ≈ 201 MB at 2,048 (needed ≈ 0.67 GB, fits).
        let m = model(12, 10, 50);
        let d = decide(
            &[dev("local", 1.0, true)],
            &m,
            &Policy::default(),
            8192,
            None,
        );
        assert_eq!(d.category, "hard");
        assert_eq!(d.needs.len(), 1);
        assert_eq!(d.needs[0].what, "context");
        assert!(d.reason.contains("2,048"), "{}", d.reason);
    }

    #[test]
    fn does_not_fit_even_pooled_at_2k_is_impossible() {
        let m = model(48, 380, 500); // ~18.7 GB, needs more than one small device
        let d = decide(
            &[dev("local", 2.0, true)],
            &m,
            &Policy::default(),
            4096,
            None,
        );
        assert_eq!(d.category, "impossible");
        assert_eq!(d.devices_used, 0);
        assert!(d.plan.is_none());
        assert!(d.reason.contains("short"), "{}", d.reason);
    }

    #[test]
    fn no_devices_online_is_impossible_with_no_need() {
        let m = model(4, 100, 50);
        let d = decide(&[], &m, &Policy::default(), 4096, None);
        assert_eq!(d.category, "impossible");
        assert!(d.reason.contains("no eligible device"), "{}", d.reason);
    }

    #[test]
    fn estimate_note_is_appended_to_the_reason() {
        let m = model(28, 20, 300);
        let d = decide(
            &[dev("local", 12.0, true)],
            &m,
            &Policy::default(),
            4096,
            Some("estimated from the catalog"),
        );
        assert_eq!(d.category, "easy");
        assert!(
            d.reason.contains("estimated from the catalog"),
            "{}",
            d.reason
        );
    }

    #[test]
    fn synth_model_info_splits_bytes_evenly_and_leaves_kv_unknown() {
        let m = synth_model_info("x.gguf", "X", 1_000_000_000, 10);
        assert_eq!(m.n_layer, 10);
        assert_eq!(m.layer_bytes.len(), 10);
        assert_eq!(m.layer_bytes[0], 100_000_000);
        assert_eq!(m.kv_bytes(4096), 0);
    }
}
