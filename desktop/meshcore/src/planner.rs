//! Placement planner.
//!
//! Rules (in order; see team notes §04 and docs/ARCHITECTURE-v3 §7):
//! 1. If the model + KV fits on one eligible device, run it there — the fastest one. No network.
//! 2. Otherwise split contiguous layer blocks across the fewest devices whose pooled free memory
//!    holds the model, filling by usable capacity. The host keeps the non-layer tensors
//!    (embeddings, output head) plus its share of layers.
//! 3. A device is rejected — with a reason — if its RTT p95 is too high, it is too hot, its
//!    battery is below the floor, or it brings less memory than one layer.
//!
//! Every placement carries a human-readable reason; the admin panel shows them verbatim.

use crate::gguf::ModelInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCap {
    pub device_id: String,
    pub name: String,
    /// Memory this device may use for weights + KV after OS/runtime headroom.
    pub usable_bytes: u64,
    /// Measured decode tokens/s on the reference micro-benchmark (0 = unknown).
    pub bench_tps: f32,
    pub rtt_ms_p95: f32,
    pub thermal_headroom: f32,
    pub battery_pct: f32,
    pub charging: bool,
    pub is_local: bool,
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
    /// Fraction for llama.cpp `--tensor-split`, host first in llama.cpp device order.
    pub split_weight: f64,
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
    pub min_thermal_headroom_margin: f32, // reject if headroom > 1 - margin (i.e. already throttling)
    pub min_battery_pct: f32,
    pub prefer_host: Option<String>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            max_rtt_ms_p95: 60.0,
            min_thermal_headroom_margin: 0.05,
            min_battery_pct: 20.0,
            prefer_host: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("no eligible devices")]
    NoDevices,
    #[error("model needs {needed} bytes but eligible devices pool only {available} bytes — add a device or use a smaller quant/context")]
    DoesNotFit { needed: u64, available: u64 },
}

fn gb(b: u64) -> String {
    format!("{:.1} GB", b as f64 / 1e9)
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

    // 1. Eligibility with reasons.
    let mut rejected: Vec<Placement> = Vec::new();
    let mut eligible: Vec<&DeviceCap> = Vec::new();
    for d in devices {
        let why = if d.rtt_ms_p95 > policy.max_rtt_ms_p95 && !d.is_local {
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
        } else if d.usable_bytes < min_layer {
            Some(format!(
                "rejected: {} usable < one layer ({})",
                gb(d.usable_bytes),
                gb(min_layer)
            ))
        } else {
            None
        };
        match why {
            Some(r) => rejected.push(Placement {
                device_id: d.device_id.clone(),
                name: d.name.clone(),
                role: Role::Rejected,
                layer_start: 0,
                layer_end: 0,
                bytes: 0,
                split_weight: 0.0,
                reason: r,
            }),
            None => eligible.push(d),
        }
    }
    if eligible.is_empty() {
        return Err(PlanError::NoDevices);
    }

    // 2. Single-device if it fits: fastest eligible device that holds everything (host preference wins ties).
    let mut single: Vec<&DeviceCap> = eligible
        .iter()
        .copied()
        .filter(|d| d.usable_bytes >= needed)
        .collect();
    // Preferred host first, then measured speed, then the local device (a tie at 0 tok/s must not
    // silently ship the model to a phone just because its id sorts first).
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
            bytes: needed,
            split_weight: 1.0,
            reason: format!(
                "fits on one device: needs {} (weights {} + KV {} @ {} ctx), has {} usable — no network in the path",
                gb(needed),
                gb(weights),
                gb(kv_total),
                n_ctx,
                gb(d.usable_bytes)
            ),
        }];
        for e in eligible.iter().filter(|e| e.device_id != d.device_id) {
            placements.push(Placement {
                device_id: e.device_id.clone(),
                name: e.name.clone(),
                role: Role::Rejected,
                layer_start: 0,
                layer_end: 0,
                bytes: 0,
                split_weight: 0.0,
                reason:
                    "not needed: model fits on the host; adding a device would only add latency"
                        .into(),
            });
        }
        placements.extend(rejected);
        return Ok(Plan {
            model: model.name.clone(),
            mode: Mode::Single,
            n_ctx,
            host_id: d.device_id.clone(),
            summary: format!(
                "{} runs entirely on {} ({} of {} usable)",
                model.name,
                d.name,
                gb(needed),
                gb(d.usable_bytes)
            ),
            placements,
            total_needed_bytes: needed,
            total_usable_bytes: d.usable_bytes,
        });
    }

    // 3. Layer split: pick the host, then add devices by usable memory (largest first) until it fits.
    let host = eligible
        .iter()
        .copied()
        .find(|d| policy.prefer_host.as_deref() == Some(&d.device_id))
        .or_else(|| eligible.iter().copied().find(|d| d.is_local))
        .unwrap_or(eligible[0]);
    let mut others: Vec<&DeviceCap> = eligible
        .iter()
        .copied()
        .filter(|d| d.device_id != host.device_id)
        .collect();
    others.sort_by_key(|d| std::cmp::Reverse(d.usable_bytes));

    let mut chosen: Vec<&DeviceCap> = vec![host];
    let mut pooled = host.usable_bytes;
    let mut it = others.iter();
    while pooled < needed {
        match it.next() {
            Some(d) => {
                chosen.push(d);
                pooled += d.usable_bytes;
            }
            None => {
                return Err(PlanError::DoesNotFit {
                    needed,
                    available: pooled,
                })
            }
        }
    }
    let unused: Vec<&DeviceCap> = it.copied().collect();

    // Host must hold the non-layer tensors; distribute layers by remaining capacity, contiguous, host first.
    let mut placements = Vec::new();
    let host_fixed = model.non_layer_bytes;
    let caps: Vec<u64> = chosen
        .iter()
        .enumerate()
        .map(|(i, d)| {
            if i == 0 {
                d.usable_bytes.saturating_sub(host_fixed)
            } else {
                d.usable_bytes
            }
        })
        .collect();
    let cap_total: u64 = caps.iter().sum();
    let layer_total: u64 = model.layer_bytes.iter().sum::<u64>() + kv_total;

    // The host must hold the non-layer tensors before it gets any layer.
    if caps[0] == 0 {
        return Err(PlanError::DoesNotFit {
            needed,
            available: pooled,
        });
    }
    let mut next_layer = 0u32;
    for (i, d) in chosen.iter().enumerate() {
        let is_last = i + 1 == chosen.len();
        // Target share proportional to capacity, but never more than the device can hold.
        let target = (layer_total as f64 * caps[i] as f64 / cap_total.max(1) as f64) as u64;
        let mut bytes = 0u64;
        let start = next_layer;
        while next_layer < n_layer {
            let lb = model.layer_bytes[next_layer as usize] + kv_per_layer;
            if !is_last && bytes + lb > target.min(caps[i]) && next_layer > start {
                break;
            }
            if bytes + lb > caps[i] {
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
            reason: if i == 0 {
                format!(
                    "host: layers {}–{} ({} layers) + embeddings/output ({}) = {} of {} usable",
                    start,
                    end.saturating_sub(1),
                    end - start,
                    gb(host_fixed),
                    gb(dev_bytes),
                    gb(d.usable_bytes)
                )
            } else {
                format!(
                    "worker: layers {}–{} ({} layers) = {} of {} usable; RTT p95 {:.0} ms",
                    start,
                    end.saturating_sub(1),
                    end - start,
                    gb(dev_bytes),
                    gb(d.usable_bytes),
                    d.rtt_ms_p95
                )
            },
        });
    }
    if next_layer < n_layer {
        return Err(PlanError::DoesNotFit {
            needed,
            available: pooled,
        });
    }
    // Normalise split weights.
    let sum: f64 = placements.iter().map(|p| p.split_weight).sum();
    for p in &mut placements {
        p.split_weight /= sum.max(1.0);
    }
    for d in unused {
        placements.push(Placement {
            device_id: d.device_id.clone(),
            name: d.name.clone(),
            role: Role::Rejected,
            layer_start: 0,
            layer_end: 0,
            bytes: 0,
            split_weight: 0.0,
            reason: "not needed: pooled memory already holds the model; fewer devices = fewer network hops".into(),
        });
    }
    placements.extend(rejected);
    let n_used = chosen.len();
    Ok(Plan {
        model: model.name.clone(),
        mode: Mode::LayerSplit,
        n_ctx,
        host_id: host.device_id.clone(),
        summary: format!(
            "{} split across {} devices: needs {} (weights {} + KV {} @ {} ctx), pooled {}",
            model.name,
            n_used,
            gb(needed),
            gb(weights),
            gb(kv_total),
            n_ctx,
            gb(pooled)
        ),
        placements,
        total_needed_bytes: needed,
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
            bench_tps: if local { 5.0 } else { 8.0 },
            rtt_ms_p95: rtt,
            thermal_headroom: 0.4,
            battery_pct: 80.0,
            charging: true,
            is_local: local,
        }
    }

    #[test]
    fn fits_on_one_device_runs_single_on_fastest() {
        let m = model(28, 20, 300); // 0.86 GB
        let p = plan(
            &m,
            &[
                dev("laptop", 12.0, true, 0.0),
                dev("phone", 6.0, false, 5.0),
            ],
            4096,
            &Policy::default(),
        )
        .unwrap();
        assert_eq!(p.mode, Mode::Single);
        assert_eq!(p.host_id, "phone"); // faster bench wins
        assert!(p
            .placements
            .iter()
            .any(|x| x.role == Role::Rejected && x.reason.starts_with("not needed")));
    }

    #[test]
    fn splits_when_it_does_not_fit_and_covers_all_layers() {
        let m = model(48, 380, 500); // ~18.7 GB like Qwen3-Coder-30B-A3B
        let p = plan(
            &m,
            &[
                dev("laptop", 12.0, true, 0.0),
                dev("phoneA", 8.5, false, 6.0),
                dev("phoneB", 8.5, false, 7.0),
            ],
            8192,
            &Policy::default(),
        )
        .unwrap();
        assert_eq!(p.mode, Mode::LayerSplit);
        let used: Vec<&Placement> = p
            .placements
            .iter()
            .filter(|x| x.role != Role::Rejected)
            .collect();
        assert_eq!(used[0].role, Role::Host);
        assert_eq!(used[0].layer_start, 0);
        let mut last = 0;
        for u in &used {
            assert_eq!(u.layer_start, last, "contiguous");
            last = u.layer_end;
        }
        assert_eq!(last, 48);
        assert!(used.len() == 2 || used.len() == 3);
        let w: f64 = used.iter().map(|u| u.split_weight).sum();
        assert!((w - 1.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_high_rtt_and_reports_reason() {
        let m = model(48, 380, 500);
        let r = plan(
            &m,
            &[
                dev("laptop", 12.0, true, 0.0),
                dev("far-phone", 8.5, false, 250.0),
            ],
            4096,
            &Policy::default(),
        );
        match r {
            Err(PlanError::DoesNotFit { .. }) => {}
            other => panic!("expected DoesNotFit, got {other:?}"),
        }
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
                dev("laptop", 12.0, true, 0.0),
                dev("phoneA", 8.5, false, 6.0),
                dev("phoneB", 8.5, false, 7.0),
            ],
            4096,
            &pol,
        )
        .unwrap();
        assert_eq!(p.host_id, "phoneA");
    }
}
