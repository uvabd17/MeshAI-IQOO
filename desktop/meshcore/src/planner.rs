//! Placement planner.
//!
//! Rules (in order; see team notes §04 and docs/MESHAI.md §10):
//! 1. If the model + KV fits on one eligible device, run it there — the fastest one. No network.
//! 2. Otherwise split contiguous layer blocks across the fewest devices whose pooled free memory
//!    holds the model. Each chosen device is filled with as many consecutive layers as it can
//!    hold (greedy, so a tight pool never fails on rounding); the host keeps the non-layer
//!    tensors (embeddings, output head) plus the first layers.
//! 3. A device is rejected — with a reason — if it is unsupported (tier gate), its RTT p95 is too
//!    high, it is too hot, its battery is below the floor, or it brings less memory than one layer
//!    plus its compute reserve.
//! 4. Every device must hold weights + KV + a fixed compute reserve (estimate, D033); the host
//!    additionally holds the non-layer tensors and, for vision models, the projector
//!    (`Policy::host_extra_bytes`). The reserves are labelled as estimates wherever they are shown.
//!
//! Every placement carries a human-readable reason; the admin panel shows them verbatim.

use crate::gguf::ModelInfo;
use serde::{Deserialize, Serialize};

/// Compute-graph memory `llama-server` needs beyond weights + KV on the device that hosts it
/// (the largest graph node, output buffers, etc.) — estimate, not measured (D016 says llama.cpp
/// compute/RPC buffers are not yet modelled). Demo-safety (D032, §17.3/§17.4): without this, the
/// planner can over-commit a device's usable memory and the process gets OOM-killed instead of
/// meshd rejecting the plan up front.
pub const HOST_COMPUTE_RESERVE_BYTES: u64 = 300_000_000; // 300 MB, estimate (D016)
/// Same, for a device that only runs `ggml-rpc-server` as a layer-split worker (its compute graph
/// is smaller: no logits, no sampling). Estimate (D016).
pub const WORKER_COMPUTE_RESERVE_BYTES: u64 = 150_000_000; // 150 MB, estimate (D016)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCap {
    pub device_id: String,
    pub name: String,
    /// Memory this device may use for weights + KV + its compute reserve (D033) after OS/runtime headroom.
    pub usable_bytes: u64,
    /// Measured decode tokens/s on the reference micro-benchmark (0 = unknown).
    pub bench_tps: f32,
    pub rtt_ms_p95: f32,
    pub thermal_headroom: f32,
    pub battery_pct: f32,
    pub charging: bool,
    pub is_local: bool,
    /// False when the device reported an unsupported tier (e.g. no i8mm on arm64, D015).
    #[serde(default = "default_true")]
    pub supported: bool,
    /// False for devices that can only be compute workers (a bare `ggml-rpc-server`, e.g. a
    /// simulated phone): they must never be chosen as the host that runs llama-server.
    #[serde(default = "default_true")]
    pub can_host: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    Host,
    Worker,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Placement {
    pub device_id: String,
    pub name: String,
    pub role: Role,
    pub layer_start: u32,
    pub layer_end: u32, // exclusive
    pub bytes: u64,
    /// Share of the offloaded bytes (informational; llama.cpp args use layer counts, see meshd).
    pub split_weight: f64,
    /// Compute-buffer estimate budgeted against this device's usable memory (0 for a rejected
    /// device); see `HOST_COMPUTE_RESERVE_BYTES` / `WORKER_COMPUTE_RESERVE_BYTES`. Separate from
    /// `bytes` (weights + KV), which is what a live run's process is credited with holding.
    pub compute_reserve_bytes: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Mode {
    Single,
    LayerSplit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub model: String,
    pub mode: Mode,
    pub n_ctx: u32,
    pub host_id: String,
    pub placements: Vec<Placement>,
    pub summary: String,
    pub total_needed_bytes: u64,
    pub total_usable_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub max_rtt_ms_p95: f32,
    pub min_thermal_headroom_margin: f32, // reject if headroom > 1 - margin (already throttling)
    pub min_battery_pct: f32,
    pub prefer_host: Option<String>,
    /// Extra bytes the host process holds beyond the model itself — today the vision projector
    /// (`--mmproj`, kept in the host process by `--no-mmproj-offload`). 0 for text models.
    pub host_extra_bytes: u64,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_rtt_ms_p95: 60.0,
            min_thermal_headroom_margin: 0.05,
            min_battery_pct: 20.0,
            prefer_host: None,
            host_extra_bytes: 0,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("no eligible devices")]
    NoDevices,
    #[error("model needs {needed} bytes (weights + KV + compute reserves, the reserves are estimates) but eligible devices pool only {available} bytes — add a device, close apps, or use a smaller quant/context")]
    DoesNotFit { needed: u64, available: u64 },
    #[error("host {host} can hold only {usable} bytes but the embeddings/output tensors plus the compute reserve alone need {fixed} bytes — pick a host with more free memory")]
    HostTooSmall {
        host: String,
        usable: u64,
        fixed: u64,
    },
    #[error("requested host {host} is not eligible: {reason}")]
    HostNotEligible { host: String, reason: String },
    #[error("no eligible device can act as host (a compute-only worker cannot run the model)")]
    NoHost,
}

fn gb(b: u64) -> String {
    format!("{:.1} GB", b as f64 / 1e9)
}

/// Reserves are shown in MB and labelled as estimates (claims discipline): "300 MB (estimate)".
fn reserve_label(b: u64) -> String {
    format!("{} MB compute reserve (estimate)", b / 1_000_000)
}

fn rejected(d: &DeviceCap, reason: String) -> Placement {
    Placement {
        device_id: d.device_id.clone(),
        name: d.name.clone(),
        role: Role::Rejected,
        layer_start: 0,
        layer_end: 0,
        bytes: 0,
        split_weight: 0.0,
        compute_reserve_bytes: 0,
        reason,
    }
}

pub fn plan(
    model: &ModelInfo,
    devices: &[DeviceCap],
    n_ctx: u32,
    policy: &Policy,
) -> Result<Plan, PlanError> {
    let kv_total = model.kv_bytes(n_ctx);
    let weights = model.weight_bytes();
    let needed = weights + kv_total;
    let n_layer = model.n_layer;
    let kv_per_layer = if n_layer > 0 {
        kv_total / n_layer as u64
    } else {
        0
    };
    let min_layer = model.layer_bytes.iter().copied().max().unwrap_or(0) + kv_per_layer;
    // A worker must hold at least one layer *and* its compute reserve (D033).
    let min_worker = min_layer + WORKER_COMPUTE_RESERVE_BYTES;
    // Bytes the host process holds beyond the model (vision projector), see Policy::host_extra_bytes.
    let host_extra = policy.host_extra_bytes;

    // 1. Eligibility with reasons.
    let mut rejects: Vec<Placement> = Vec::new();
    let mut eligible: Vec<&DeviceCap> = Vec::new();
    for d in devices {
        let why = if !d.supported {
            Some(
                "rejected: unsupported device (needs arm64 with dotprod + i8mm and ≥ 8 GB, D015)"
                    .to_string(),
            )
        } else if d.rtt_ms_p95 > policy.max_rtt_ms_p95 && !d.is_local {
            Some(format!(
                "rejected: RTT p95 {:.0} ms > {:.0} ms limit",
                d.rtt_ms_p95, policy.max_rtt_ms_p95
            ))
        } else if d.thermal_headroom > 1.0 - policy.min_thermal_headroom_margin {
            Some(format!(
                "rejected: thermal headroom {:.2} (throttling)",
                d.thermal_headroom
            ))
        } else if !d.charging && d.battery_pct < policy.min_battery_pct && !d.is_local {
            Some(format!(
                "rejected: battery {:.0}% < {:.0}% floor and not charging",
                d.battery_pct, policy.min_battery_pct
            ))
        } else if d.usable_bytes < min_worker {
            Some(format!(
                "rejected: {} usable < one layer ({}) + {}",
                gb(d.usable_bytes),
                gb(min_layer),
                reserve_label(WORKER_COMPUTE_RESERVE_BYTES)
            ))
        } else {
            None
        };
        match why {
            Some(r) => rejects.push(rejected(d, r)),
            None => eligible.push(d),
        }
    }
    if eligible.is_empty() {
        return Err(PlanError::NoDevices);
    }
    // A requested host must be eligible and host-capable; never fall back silently to another
    // device, because the user asked for this one (a capped laptop once made the planner pick a
    // simulated worker as host, which can never serve).
    if let Some(want) = policy.prefer_host.as_deref() {
        match eligible.iter().find(|d| d.device_id == want) {
            None => {
                let reason = rejects
                    .iter()
                    .find(|p| p.device_id == want)
                    .map(|p| p.reason.clone())
                    .unwrap_or_else(|| "unknown or offline device".into());
                return Err(PlanError::HostNotEligible {
                    host: want.into(),
                    reason,
                });
            }
            Some(d) if !d.can_host => {
                return Err(PlanError::HostNotEligible {
                    host: want.into(),
                    reason: "compute-only worker (cannot run llama-server)".into(),
                })
            }
            _ => {}
        }
    }
    if !eligible.iter().any(|d| d.can_host) {
        return Err(PlanError::NoHost);
    }

    // 2. Single-device if it fits: preferred host first, then measured speed, then the local
    //    device (a tie at 0 tok/s must not ship the model to a phone because its id sorts first).
    //    The single device also runs llama-server, so it must clear the host compute reserve.
    let needed_with_reserve = needed + host_extra + HOST_COMPUTE_RESERVE_BYTES;
    let mut single: Vec<&DeviceCap> = eligible
        .iter()
        .copied()
        .filter(|d| d.can_host && d.usable_bytes >= needed_with_reserve)
        .filter(|d| {
            policy
                .prefer_host
                .as_deref()
                .is_none_or(|h| h == d.device_id)
        })
        .collect();
    single.sort_by(|a, b| {
        let pa = policy.prefer_host.as_deref() == Some(&a.device_id);
        let pb = policy.prefer_host.as_deref() == Some(&b.device_id);
        pb.cmp(&pa)
            .then(
                b.bench_tps
                    .partial_cmp(&a.bench_tps)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(b.is_local.cmp(&a.is_local))
    });
    if let Some(d) = single.first() {
        let mut placements = vec![Placement {
            device_id: d.device_id.clone(),
            name: d.name.clone(),
            role: Role::Host,
            layer_start: 0,
            layer_end: n_layer,
            bytes: needed + host_extra,
            split_weight: 1.0,
            compute_reserve_bytes: HOST_COMPUTE_RESERVE_BYTES,
            reason: format!(
                "fits on one device: needs {} (weights {} + KV {} @ {} ctx){} + {} = {}, has {} usable — no network in the path",
                gb(needed), gb(weights), gb(kv_total), n_ctx,
                if host_extra > 0 { format!(" + projector {}", gb(host_extra)) } else { String::new() },
                reserve_label(HOST_COMPUTE_RESERVE_BYTES), gb(needed_with_reserve), gb(d.usable_bytes)
            ),
        }];
        for e in eligible.iter().filter(|e| e.device_id != d.device_id) {
            placements.push(rejected(
                e,
                "not needed: model fits on the host; adding a device would only add latency".into(),
            ));
        }
        placements.extend(rejects);
        return Ok(Plan {
            model: model.name.clone(),
            mode: Mode::Single,
            n_ctx,
            host_id: d.device_id.clone(),
            summary: format!(
                "{} runs entirely on {} ({} of {} usable, incl. {})",
                model.name,
                d.name,
                gb(needed_with_reserve),
                gb(d.usable_bytes),
                reserve_label(HOST_COMPUTE_RESERVE_BYTES)
            ),
            placements,
            total_needed_bytes: needed_with_reserve,
            total_usable_bytes: d.usable_bytes,
        });
    }

    // 3. Layer split: pick the host, then add devices by usable memory (largest first) until the
    //    pool holds the model. The host must at least hold the non-layer tensors.
    let hosts: Vec<&DeviceCap> = eligible.iter().copied().filter(|d| d.can_host).collect();
    let host = hosts
        .iter()
        .copied()
        .find(|d| policy.prefer_host.as_deref() == Some(&d.device_id))
        .or_else(|| hosts.iter().copied().find(|d| d.is_local))
        .unwrap_or(hosts[0]);
    let host_fixed = model.non_layer_bytes + host_extra;
    // The host must also clear its compute reserve before it can take even the fixed tensors.
    let host_fixed_with_reserve = host_fixed + HOST_COMPUTE_RESERVE_BYTES;
    if host.usable_bytes <= host_fixed_with_reserve {
        return Err(PlanError::HostTooSmall {
            host: host.name.clone(),
            usable: host.usable_bytes,
            fixed: host_fixed_with_reserve,
        });
    }
    let mut others: Vec<&DeviceCap> = eligible
        .iter()
        .copied()
        .filter(|d| d.device_id != host.device_id)
        .collect();
    others.sort_by_key(|d| std::cmp::Reverse(d.usable_bytes));

    // Greedy contiguous fill (D023): each device takes as many consecutive layers as it can hold;
    // devices are added largest-first until every layer is placed, so whole-layer fragmentation
    // never produces a false DoesNotFit while an eligible device is still unused.
    let mut placements = Vec::new();
    let mut next_layer = 0u32;
    let mut pooled = 0u64;
    let mut it = others.iter();
    let mut i = 0usize;
    let mut d: &DeviceCap = host;
    loop {
        // The compute reserve comes off the top of what a device can take, alongside the fixed
        // tensors on the host (D032, §17.4): a device must hold weights + KV + its reserve.
        let reserve = if i == 0 {
            HOST_COMPUTE_RESERVE_BYTES
        } else {
            WORKER_COMPUTE_RESERVE_BYTES
        };
        let cap = if i == 0 {
            d.usable_bytes
                .saturating_sub(host_fixed)
                .saturating_sub(reserve)
        } else {
            d.usable_bytes.saturating_sub(reserve)
        };
        pooled += d.usable_bytes;
        let mut bytes = 0u64;
        let start = next_layer;
        while next_layer < n_layer {
            let lb = model.layer_bytes[next_layer as usize] + kv_per_layer;
            if bytes + lb > cap {
                break; // never assign a layer the device cannot hold (M5), even as its first
            }
            bytes += lb;
            next_layer += 1;
        }
        let end = next_layer;
        let dev_bytes = bytes + if i == 0 { host_fixed } else { 0 };
        placements.push(Placement {
            device_id: d.device_id.clone(),
            name: d.name.clone(),
            role: if i == 0 { Role::Host } else { Role::Worker },
            layer_start: start,
            layer_end: end,
            bytes: dev_bytes,
            split_weight: dev_bytes as f64,
            compute_reserve_bytes: reserve,
            reason: if i == 0 {
                format!(
                    "host: layers {}–{} ({} layers) + embeddings/output{} ({}) + {} = {} of {} usable",
                    start,
                    end.saturating_sub(1),
                    end - start,
                    if host_extra > 0 { "/projector" } else { "" },
                    gb(host_fixed),
                    reserve_label(reserve),
                    gb(dev_bytes + reserve),
                    gb(d.usable_bytes)
                )
            } else {
                format!(
                    "worker: layers {}–{} ({} layers) + {} = {} of {} usable; RTT p95 {:.0} ms",
                    start,
                    end.saturating_sub(1),
                    end - start,
                    reserve_label(reserve),
                    gb(dev_bytes + reserve),
                    gb(d.usable_bytes),
                    d.rtt_ms_p95
                )
            },
        });
        if next_layer >= n_layer {
            break;
        }
        match it.next() {
            Some(n) => {
                d = n;
                i += 1;
            }
            None => {
                // Reserves are part of what must fit: host + one per worker used so far (estimates).
                let reserves = HOST_COMPUTE_RESERVE_BYTES + WORKER_COMPUTE_RESERVE_BYTES * i as u64;
                return Err(PlanError::DoesNotFit {
                    needed: needed + host_extra + reserves,
                    available: pooled,
                });
            }
        }
    }
    let unused: Vec<&DeviceCap> = it.copied().collect();
    // A worker that ended up with zero layers is not needed after all.
    placements.retain(|p| p.role == Role::Host || p.layer_end > p.layer_start);
    let sum: f64 = placements.iter().map(|p| p.split_weight).sum();
    for p in &mut placements {
        p.split_weight /= sum.max(1.0);
    }
    let n_used = placements.len();
    for d in unused {
        placements.push(rejected(
            d,
            "not needed: pooled memory already holds the model; fewer devices = fewer network hops"
                .into(),
        ));
    }
    placements.extend(rejects);
    Ok(Plan {
        model: model.name.clone(),
        mode: Mode::LayerSplit,
        n_ctx,
        host_id: host.device_id.clone(),
        summary: format!(
            "{} split across {} devices: needs {} (weights {} + KV {} @ {} ctx{}) + compute reserves (estimate), pooled {}",
            model.name,
            n_used,
            gb(needed + host_extra),
            gb(weights),
            gb(kv_total),
            n_ctx,
            if host_extra > 0 { format!(" + projector {}", gb(host_extra)) } else { String::new() },
            gb(pooled)
        ),
        placements,
        total_needed_bytes: needed
            + host_extra
            + HOST_COMPUTE_RESERVE_BYTES
            + WORKER_COMPUTE_RESERVE_BYTES * (n_used.saturating_sub(1)) as u64,
        total_usable_bytes: pooled,
    })
}

#[cfg(test)]
mod tests {
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
            kv_bytes_per_token: 48 * 2 * 4 * 128 * 2, // ~96 KB
            tensor_count: 0,
        }
    }
    fn dev(id: &str, gb: f64, local: bool, rtt: f32) -> DeviceCap {
        DeviceCap {
            device_id: id.into(),
            name: id.into(),
            usable_bytes: (gb * 1e9) as u64,
            bench_tps: 0.0,
            rtt_ms_p95: rtt,
            thermal_headroom: 0.4,
            battery_pct: 80.0,
            charging: true,
            is_local: local,
            supported: true,
            can_host: true,
        }
    }

    fn worker_only(id: &str, gb: f64) -> DeviceCap {
        let mut d = dev(id, gb, false, 1.0);
        d.can_host = false;
        d
    }

    #[test]
    fn a_compute_only_worker_is_never_the_host() {
        // Laptop too small to hold the model alone, sim big enough: the sim must NOT become host.
        let m = model(12, 100, 50);
        let p = plan(
            &m,
            &[dev("local", 0.5, true, 0.0), worker_only("sim", 3.0)],
            1,
            &Policy::default(),
        )
        .unwrap();
        assert_eq!(p.host_id, "local");
        assert_eq!(p.mode, Mode::LayerSplit);
        // Only worker-only devices online: no host at all.
        let e = plan(&m, &[worker_only("sim", 3.0)], 1, &Policy::default()).unwrap_err();
        assert!(matches!(e, PlanError::NoHost));
    }

    #[test]
    fn a_requested_host_is_honoured_or_refused_never_swapped() {
        let m = model(12, 100, 50);
        let pol = Policy {
            prefer_host: Some("local".into()),
            ..Policy::default()
        };
        // Fits on the phone alone, but the user asked for the laptop: split with the laptop as host.
        let p = plan(
            &m,
            &[dev("local", 0.5, true, 0.0), dev("phone", 3.0, false, 1.0)],
            1,
            &pol,
        )
        .unwrap();
        assert_eq!(p.host_id, "local");
        // The requested host is rejected (RTT too high): an explicit error, not another host.
        let bad = Policy {
            prefer_host: Some("phone".into()),
            ..Policy::default()
        };
        let e = plan(
            &m,
            &[
                dev("local", 3.0, true, 0.0),
                dev("phone", 3.0, false, 999.0),
            ],
            1,
            &bad,
        )
        .unwrap_err();
        assert!(matches!(e, PlanError::HostNotEligible { .. }), "{e}");
    }
    fn used(p: &Plan) -> Vec<&Placement> {
        p.placements
            .iter()
            .filter(|x| x.role != Role::Rejected)
            .collect()
    }
    fn contiguous(p: &Plan, n: u32) {
        let u = used(p);
        assert_eq!(u[0].role, Role::Host);
        let mut last = 0;
        for x in &u {
            assert_eq!(x.layer_start, last, "contiguous");
            last = x.layer_end;
        }
        assert_eq!(last, n);
    }

    #[test]
    fn fits_on_one_device_prefers_faster_then_local() {
        let m = model(28, 20, 300); // 0.86 GB
        let mut phone = dev("aaaa-phone", 6.0, false, 5.0);
        phone.bench_tps = 8.0;
        let laptop = dev("local", 12.0, true, 0.0);
        let p = plan(
            &m,
            &[laptop.clone(), phone.clone()],
            4096,
            &Policy::default(),
        )
        .unwrap();
        assert_eq!(p.mode, Mode::Single);
        assert_eq!(p.host_id, "aaaa-phone", "measured speed wins");
        // With no measurements the local device wins the tie, even though "aaaa-phone" sorts first.
        phone.bench_tps = 0.0;
        let p = plan(&m, &[phone, laptop], 4096, &Policy::default()).unwrap();
        assert_eq!(p.host_id, "local");
    }

    #[test]
    fn splits_when_it_does_not_fit_and_covers_all_layers() {
        let m = model(48, 380, 500); // ~18.7 GB like Qwen3-Coder-30B-A3B
        let p = plan(
            &m,
            &[
                dev("local", 12.0, true, 0.0),
                dev("phoneA", 8.5, false, 6.0),
                dev("phoneB", 8.5, false, 7.0),
            ],
            8192,
            &Policy::default(),
        )
        .unwrap();
        assert_eq!(p.mode, Mode::LayerSplit);
        contiguous(&p, 48);
        let w: f64 = used(&p).iter().map(|u| u.split_weight).sum();
        assert!((w - 1.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_high_rtt_and_unsupported_with_reasons() {
        let m = model(48, 380, 500);
        let mut bad = dev("old-phone", 8.5, false, 5.0);
        bad.supported = false;
        let r = plan(
            &m,
            &[
                dev("local", 12.0, true, 0.0),
                dev("far-phone", 8.5, false, 250.0),
                bad,
            ],
            4096,
            &Policy::default(),
        );
        match r {
            Err(PlanError::DoesNotFit { .. }) => {}
            other => panic!("expected DoesNotFit, got {other:?}"),
        }
        // The same devices with a model that fits the laptop: the rejections are reported.
        let small = model(28, 20, 300);
        let mut bad = dev("old-phone", 8.5, false, 5.0);
        bad.supported = false;
        let p = plan(
            &small,
            &[dev("local", 12.0, true, 0.0), bad],
            4096,
            &Policy::default(),
        )
        .unwrap();
        let rej = p
            .placements
            .iter()
            .find(|x| x.device_id == "old-phone")
            .unwrap();
        assert!(rej.reason.contains("unsupported"), "{}", rej.reason);
    }

    #[test]
    fn prefer_host_is_honoured_in_split() {
        let m = model(48, 380, 500);
        let pol = Policy {
            prefer_host: Some("phoneA".into()),
            ..Default::default()
        };
        let p = plan(
            &m,
            &[
                dev("local", 12.0, true, 0.0),
                dev("phoneA", 8.5, false, 6.0),
                dev("phoneB", 8.5, false, 7.0),
            ],
            4096,
            &pol,
        )
        .unwrap();
        assert_eq!(p.host_id, "phoneA");
    }

    #[test]
    fn never_assigns_a_layer_a_device_cannot_hold() {
        // Host holds embeddings + 1 layer; "tiny" holds less than one layer → rejected; "big" (too small
        // for the whole model, so no single-device shortcut) takes the remaining 9 layers.
        // Capacities include the compute reserve (D032): host needs host_fixed (50 MB) + the host
        // reserve (300 MB) + 1 layer (≈105 MB) headroom; "big" needs 9 layers (≈945 MB) + the
        // worker reserve (150 MB) headroom.
        let m = model(10, 100, 50); // 1.05 GB + ~50 MB KV
        let p = plan(
            &m,
            &[
                dev("local", 0.48, true, 0.0),
                dev("tiny", 0.05, false, 1.0),
                dev("big", 1.1, false, 1.0),
            ],
            512,
            &Policy::default(),
        )
        .unwrap();
        contiguous(&p, 10);
        for u in used(&p) {
            let cap: u64 = if u.role == Role::Host {
                480_000_000
            } else {
                1_100_000_000
            };
            // Weights + KV *and* the device's compute reserve must fit (D033).
            assert!(
                u.bytes + u.compute_reserve_bytes <= cap + 1,
                "{} over cap incl. reserve",
                u.name
            );
        }
        let tiny = p.placements.iter().find(|x| x.device_id == "tiny").unwrap();
        assert_eq!(tiny.role, Role::Rejected);
    }

    #[test]
    fn compute_reserve_costs_a_device_its_last_layer() {
        // A phone that could hold 5 layers on capacity alone (550 MB / 100 MB = 5.5 → 5) can hold
        // only 4 once the worker compute reserve (150 MB, D032) is taken off the top: 550 MB -
        // 150 MB = 400 MB → 4 layers. The remainder (1 layer) must go to a second device instead
        // of being dropped or over-committing the first.
        let mut m = model(6, 100, 0); // 100 MB/layer, no fixed tensors, so host_fixed is trivial
        m.kv_bytes_per_token = 0;
        let p = plan(
            &m,
            &[
                dev("local", 0.4, true, 0.0), // host_fixed(0) + reserve(300 MB) + 1 layer = 400 MB
                dev("phoneA", 0.55, false, 1.0), // would hold 5 layers without the reserve
                dev("phoneB", 0.3, false, 1.0), // picks up the layer the reserve costs phoneA
            ],
            1,
            &Policy::default(),
        )
        .unwrap();
        contiguous(&p, 6);
        let a = p
            .placements
            .iter()
            .find(|x| x.device_id == "phoneA")
            .unwrap();
        assert_eq!(
            a.layer_end - a.layer_start,
            4,
            "550 MB usable minus the 150 MB worker reserve fits only 4 of the 5 layers capacity alone would suggest"
        );
        assert_eq!(a.compute_reserve_bytes, WORKER_COMPUTE_RESERVE_BYTES);
        let b = p
            .placements
            .iter()
            .find(|x| x.device_id == "phoneB")
            .unwrap();
        assert_eq!(
            b.role,
            Role::Worker,
            "the layer the reserve displaced must land somewhere, not vanish"
        );
        assert_eq!(b.layer_end - b.layer_start, 1);
    }

    #[test]
    fn single_device_needs_the_host_reserve_too() {
        // needed = 6 × 100 MB + 50 MB (KV 0) = 0.65 GB. A lone device with 0.85 GB has the model plus
        // 200 MB of slack — less than the 300 MB host reserve — so it must NOT be planned as Single
        // (that is exactly the over-commit an OOM kill comes from, D033).
        let mut m = model(6, 100, 50);
        m.kv_bytes_per_token = 0;
        let r = plan(&m, &[dev("local", 0.85, true, 0.0)], 1, &Policy::default());
        assert!(
            !matches!(r, Ok(ref p) if p.mode == Mode::Single),
            "0.85 GB must not host 0.65 GB + 300 MB reserve as a single device: {r:?}"
        );
        // With the reserve covered it is Single again.
        let p = plan(&m, &[dev("local", 0.96, true, 0.0)], 1, &Policy::default()).unwrap();
        assert_eq!(p.mode, Mode::Single);
        assert_eq!(
            p.total_needed_bytes,
            650_000_000 + HOST_COMPUTE_RESERVE_BYTES
        );
    }

    #[test]
    fn projector_is_budgeted_on_the_host() {
        // Same 0.65 GB text model fits a 1.0 GB device alone; with a 0.4 GB vision projector that
        // stays in the host process (--no-mmproj-offload) it no longer does, and the projector must
        // also count in the split host's fixed bytes.
        let mut m = model(6, 100, 50);
        m.kv_bytes_per_token = 0;
        let pol = Policy {
            host_extra_bytes: 400_000_000,
            ..Policy::default()
        };
        let p = plan(&m, &[dev("local", 1.0, true, 0.0)], 1, &Policy::default()).unwrap();
        assert_eq!(p.mode, Mode::Single);
        let r = plan(&m, &[dev("local", 1.0, true, 0.0)], 1, &pol);
        assert!(!matches!(r, Ok(ref p) if p.mode == Mode::Single), "{r:?}");
        // Add a worker: host holds projector + head + reserve + layers, worker takes the rest.
        let p = plan(
            &m,
            &[dev("local", 1.0, true, 0.0), dev("w", 0.6, false, 1.0)],
            1,
            &pol,
        )
        .unwrap();
        assert_eq!(p.mode, Mode::LayerSplit);
        let host = p.placements.iter().find(|x| x.role == Role::Host).unwrap();
        assert!(
            host.bytes >= 450_000_000,
            "host bytes include the projector: {}",
            host.bytes
        );
        assert!(host.bytes + host.compute_reserve_bytes <= 1_000_000_000);
        assert!(host.reason.contains("projector"), "{}", host.reason);
    }

    #[test]
    fn fragmentation_adds_the_next_device_instead_of_failing() {
        // 12 × 100 MB + 50 MB head, KV 0 → needed 1.25 GB. Capacities include the compute reserve
        // (D033) *plus 40 MB of whole-layer waste each*: local 0.69 GB (host_fixed 50 MB + host
        // reserve 300 MB + 3 layers + 40 MB), a/b 0.59 GB (worker reserve 150 MB + 4 layers + 40 MB).
        // Raw pooled memory (1.87 GB) exceeds needed + reserves (1.85 GB), yet by whole layers
        // local+a+b hold only 11: c (0.30 GB = worker reserve + 1 layer) must be added for the 12th.
        let mut m = model(12, 100, 50);
        m.kv_bytes_per_token = 0;
        let p = plan(
            &m,
            &[
                dev("local", 0.69, true, 0.0),
                dev("a", 0.59, false, 1.0),
                dev("b", 0.59, false, 1.0),
                dev("c", 0.30, false, 1.0),
            ],
            1,
            &Policy::default(),
        )
        .unwrap();
        contiguous(&p, 12);
        assert_eq!(used(&p).len(), 4);
    }

    #[test]
    fn tight_pool_still_fits_thanks_to_greedy_fill() {
        // 12 layers of 100 MB + 50 MB head. Capacities include the compute reserve (D032): host
        // 0.65 GB (host_fixed 50 MB + host reserve 300 MB + 3 layers), a 0.55 GB (worker reserve
        // 150 MB + 4 layers), b 0.65 GB (worker reserve 150 MB + 5 layers) = exactly 12 layers.
        let mut m = model(12, 100, 50);
        m.kv_bytes_per_token = 0;
        let p = plan(
            &m,
            &[
                dev("local", 0.65, true, 0.0),
                dev("a", 0.55, false, 1.0),
                dev("b", 0.65, false, 1.0),
            ],
            1,
            &Policy::default(),
        )
        .unwrap();
        contiguous(&p, 12);
        let counts: Vec<u32> = used(&p)
            .iter()
            .map(|u| u.layer_end - u.layer_start)
            .collect();
        // devices are added largest-first after the host: local(3), b(5), a(4)
        assert_eq!(counts, vec![3, 5, 4]);
    }
}
