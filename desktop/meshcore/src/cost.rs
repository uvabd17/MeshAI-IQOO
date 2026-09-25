//! Cost model for the Calculate step (docs/MESHAI.md §17.4, D032, task T074).
//!
//! Pure functions: given the stacks a plan puts on each device, the devices' measured (or
//! estimated) effective bandwidth and compute, and the per-link costs, predict the time of one
//! request. Every output carries a provenance tag so the panel can show `m`/`e`/`?` honestly.
//!
//! ```text
//! decode ms/token  t_d = max( bytes read per token / BW_d , FLOPs per token / G_d )
//! link per worker  h_w = alpha_w + 2 × activation / beta_w
//! token time       T   = Σ t_d + Σ h_w + overhead
//! prefill          TTFT = Σ_d max( P × FLOPs_d / G_d , ⌈P/ubatch⌉ × weights_d / BW_d )
//!                         + Σ_w ( ⌈P/ubatch⌉ × alpha_w + P × activation / beta_w )
//! answer           total = TTFT + (N − 1) × T
//! ```
//! Not modelled (errs slow): pipeline overlap between micro-batches, thermal throttling.

use serde::{Deserialize, Serialize};

/// Where a number came from. `Unknown` means "show no number, offer a Measure button".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Src {
    Measured,
    Derived,
    Estimate,
    Unknown,
}

impl Src {
    /// The weakest of two sources (a value derived from an estimate is an estimate).
    pub fn weakest(self, other: Src) -> Src {
        use Src::*;
        match (self, other) {
            (Unknown, _) | (_, Unknown) => Unknown,
            (Estimate, _) | (_, Estimate) => Estimate,
            (Derived, _) | (_, Derived) => Derived,
            _ => Measured,
        }
    }
    /// Display band (± fraction) per §17.4: measured 15 %, derived 30 %, estimate 50 %.
    pub fn band(self) -> f64 {
        match self {
            Src::Measured => 0.15,
            Src::Derived => 0.30,
            Src::Estimate => 0.50,
            Src::Unknown => 1.0,
        }
    }
}

/// A value with its provenance and display band.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Est {
    pub v: f64,
    pub lo: f64,
    pub hi: f64,
    pub src: Src,
}

impl Est {
    pub fn new(v: f64, src: Src) -> Est {
        let b = src.band();
        Est {
            v,
            lo: v * (1.0 - b),
            hi: v * (1.0 + b),
            src,
        }
    }
    pub fn unknown() -> Est {
        Est {
            v: 0.0,
            lo: 0.0,
            hi: 0.0,
            src: Src::Unknown,
        }
    }
    pub fn is_known(&self) -> bool {
        self.src != Src::Unknown
    }
}

/// A device's effective speeds. `bw_bytes_per_s` is what decode actually achieves (backed out of
/// a measured single-device run: bytes per token × tok/s), `gflops` what prefill achieves.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevicePerf {
    pub device_id: String,
    pub bw_bytes_per_s: f64,
    pub bw_src: Src,
    pub gflops: f64,
    pub gflops_src: Src,
}

/// What a plan puts on one device: the units' totals (from `layers::LayerConfig::stack` plus the
/// host's fixed units) and the KV bytes per context token held there.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackInput {
    pub device_id: String,
    pub is_host: bool,
    pub layer_start: u32,
    pub layer_end: u32,
    pub weight_bytes: u64,
    pub read_bytes_per_token: u64,
    pub flops_per_token: u64,
    pub kv_bytes_per_token: u64,
}

/// Per-link cost for one worker: a fixed per-message cost (RTT plus relay overhead) and a bulk
/// rate. On the adb relay the fixed cost dominates (≈0.7 s per token measured, §17.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkInput {
    pub device_id: String,
    pub alpha_ms: f64,
    pub alpha_src: Src,
    pub beta_bytes_per_s: f64,
    pub beta_src: Src,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Request {
    pub prompt_tokens: u32,
    pub answer_tokens: u32,
    /// Activation bytes crossing a device boundary per token (n_embd × 4).
    pub activation_bytes: u64,
    /// llama-server micro-batch (default 512).
    pub ubatch: u32,
}

impl Default for Request {
    fn default() -> Self {
        Request {
            prompt_tokens: 500,
            answer_tokens: 200,
            activation_bytes: 4096 * 4,
            ubatch: 512,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StackCost {
    pub device_id: String,
    pub is_host: bool,
    pub layer_start: u32,
    pub layer_end: u32,
    pub weight_bytes: u64,
    pub read_bytes_per_token: u64,
    pub gflop_per_token: f64,
    /// Decode cost of this stack per token at the middle of the answer.
    pub decode_ms: Est,
    /// Prefill cost of this stack for the whole prompt.
    pub prefill_ms: Est,
    pub bottleneck: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkCost {
    pub device_id: String,
    pub decode_ms: Est,
    pub prefill_ms: Est,
}

#[derive(Debug, Clone, Serialize)]
pub struct Prediction {
    pub stacks: Vec<StackCost>,
    pub links: Vec<LinkCost>,
    /// Milliseconds per generated token.
    pub token_ms: Est,
    pub tok_s: Est,
    pub ttft_ms: Est,
    pub total_ms: Est,
    pub bottleneck: String,
    pub warnings: Vec<String>,
}

/// Fixed per-token overhead outside the graph (sampling, HTTP): estimate.
pub const OVERHEAD_MS: f64 = 2.0;

fn find_perf<'a>(perf: &'a [DevicePerf], id: &str) -> Option<&'a DevicePerf> {
    perf.iter().find(|p| p.device_id == id)
}

/// Predict one request. Stacks with a device that has no perf entry make the result `Unknown`.
pub fn predict(
    req: &Request,
    stacks: &[StackInput],
    perf: &[DevicePerf],
    links: &[LinkInput],
) -> Prediction {
    let mut warnings = Vec::new();
    let p = req.prompt_tokens.max(1) as f64;
    let n = req.answer_tokens.max(1) as f64;
    let ubatches = (req.prompt_tokens as f64 / req.ubatch.max(1) as f64)
        .ceil()
        .max(1.0);
    // Decode is costed at the middle of the answer: the KV read grows with position.
    let pos = p + n / 2.0;

    let mut token_ms = 0.0;
    let mut ttft_ms = 0.0;
    let mut src = Src::Measured;
    let mut worst: (f64, String) = (0.0, String::new());
    let mut out = Vec::with_capacity(stacks.len());
    for s in stacks {
        let Some(pf) = find_perf(perf, &s.device_id) else {
            warnings.push(format!("no speed measurement for {}", s.device_id));
            src = Src::Unknown;
            out.push(StackCost {
                device_id: s.device_id.clone(),
                is_host: s.is_host,
                layer_start: s.layer_start,
                layer_end: s.layer_end,
                weight_bytes: s.weight_bytes,
                read_bytes_per_token: s.read_bytes_per_token,
                gflop_per_token: s.flops_per_token as f64 / 1e9,
                decode_ms: Est::unknown(),
                prefill_ms: Est::unknown(),
                bottleneck: "unknown",
            });
            continue;
        };
        let bw = pf.bw_bytes_per_s.max(1.0);
        let g = pf.gflops.max(1e-6) * 1e9;
        // decode: bytes read (weights touched + KV so far) vs compute
        let read = s.read_bytes_per_token as f64 + pos * s.kv_bytes_per_token as f64;
        let t_mem = read / bw * 1000.0;
        let t_cmp = s.flops_per_token as f64 / g * 1000.0;
        let (t_dec, bn) = if t_mem >= t_cmp {
            (t_mem, "memory bandwidth")
        } else {
            (t_cmp, "compute")
        };
        // prefill: compute over the prompt (attention over past tokens is folded into a 1.15
        // factor, estimate) vs re-reading the weights once per micro-batch
        let pf_cmp = p * s.flops_per_token as f64 * 1.15 / g * 1000.0;
        let pf_mem = ubatches * s.weight_bytes as f64 / bw * 1000.0;
        let t_pf = pf_cmp.max(pf_mem);
        let dsrc = pf.bw_src.weakest(pf.gflops_src);
        src = src.weakest(dsrc);
        token_ms += t_dec;
        ttft_ms += t_pf;
        if t_dec > worst.0 {
            worst = (t_dec, format!("{} {}", s.device_id, bn));
        }
        out.push(StackCost {
            device_id: s.device_id.clone(),
            is_host: s.is_host,
            layer_start: s.layer_start,
            layer_end: s.layer_end,
            weight_bytes: s.weight_bytes,
            read_bytes_per_token: s.read_bytes_per_token,
            gflop_per_token: s.flops_per_token as f64 / 1e9,
            decode_ms: Est::new(t_dec, dsrc),
            prefill_ms: Est::new(t_pf, pf.gflops_src.weakest(Src::Derived)),
            bottleneck: bn,
        });
    }

    let mut link_out = Vec::new();
    for s in stacks.iter().filter(|s| !s.is_host) {
        let Some(l) = links.iter().find(|l| l.device_id == s.device_id) else {
            warnings.push(format!("no link measurement for {}", s.device_id));
            src = Src::Unknown;
            link_out.push(LinkCost {
                device_id: s.device_id.clone(),
                decode_ms: Est::unknown(),
                prefill_ms: Est::unknown(),
            });
            continue;
        };
        let beta = l.beta_bytes_per_s.max(1.0);
        let a = req.activation_bytes as f64;
        // star topology: one activation in and one out per token (§17.3 F5)
        let h_dec = l.alpha_ms + 2.0 * a / beta * 1000.0;
        let h_pf = ubatches * l.alpha_ms + 2.0 * p * a / beta * 1000.0;
        let lsrc = l.alpha_src.weakest(l.beta_src);
        src = src.weakest(lsrc);
        token_ms += h_dec;
        ttft_ms += h_pf;
        if h_dec > worst.0 {
            worst = (h_dec, format!("link to {}", s.device_id));
        }
        link_out.push(LinkCost {
            device_id: s.device_id.clone(),
            decode_ms: Est::new(h_dec, lsrc),
            prefill_ms: Est::new(h_pf, lsrc),
        });
    }

    if src == Src::Unknown {
        return Prediction {
            stacks: out,
            links: link_out,
            token_ms: Est::unknown(),
            tok_s: Est::unknown(),
            ttft_ms: Est::unknown(),
            total_ms: Est::unknown(),
            bottleneck: "unknown — run once to measure".into(),
            warnings,
        };
    }
    token_ms += OVERHEAD_MS;
    ttft_ms += OVERHEAD_MS;
    let total = ttft_ms + (n - 1.0) * token_ms;
    Prediction {
        stacks: out,
        links: link_out,
        token_ms: Est::new(token_ms, src),
        tok_s: Est::new(1000.0 / token_ms, src),
        ttft_ms: Est::new(ttft_ms, src),
        total_ms: Est::new(total, src),
        bottleneck: worst.1,
        warnings,
    }
}

/// Back out a device's effective decode bandwidth from a measured single-device run:
/// bytes touched per token (read bytes + KV at the run's average position) × tok/s.
pub fn bandwidth_from_run(read_bytes_per_token: u64, kv_bytes_at_pos: u64, tok_s: f64) -> f64 {
    (read_bytes_per_token + kv_bytes_at_pos) as f64 * tok_s
}

/// Back out effective prefill GFLOP/s from a measured prompt rate.
pub fn gflops_from_run(flops_per_token: u64, prompt_tok_s: f64) -> f64 {
    flops_per_token as f64 * 1.15 * prompt_tok_s / 1e9
}

/// Back out the fixed link cost per token from a measured split: the leftover after the stacks'
/// predicted compute. Returns None when the split was faster than its compute alone (bad inputs).
pub fn alpha_from_split(measured_token_ms: f64, predicted_compute_ms: f64) -> Option<f64> {
    let a = measured_token_ms - predicted_compute_ms - OVERHEAD_MS;
    (a >= 0.0).then_some(a)
}

#[cfg(test)]
mod tests {
    use super::*;

    // §17.4 worked example, Qwen3-8B Q4_K_M: 36 blocks of 115.8 MB (4 KB KV each per token),
    // embd 350 MB + head 510.5 MB on the host; laptop BW 14.3 GB/s / 122 GFLOP/s [E←M],
    // phone BW 18.2 GB/s / 81 GFLOP/s [E←M], adb alpha 717 ms [E←M].
    const BLK: u64 = 115_800_000;
    const BLK_FLOPS: u64 = 385_892_864;
    const KV: u64 = 4096;
    const EMBD: u64 = 350_000_000;
    const HEAD: u64 = 510_500_000;
    const HEAD_FLOPS: u64 = 1_244_667_904;

    fn stack(id: &str, host: bool, a: u32, b: u32) -> StackInput {
        let n = (b - a) as u64;
        let fixed_bytes = if host { EMBD + HEAD } else { 0 };
        let fixed_read = if host { HEAD } else { 0 }; // the embedding is a row lookup
        let fixed_flops = if host { HEAD_FLOPS } else { 0 };
        StackInput {
            device_id: id.into(),
            is_host: host,
            layer_start: a,
            layer_end: b,
            weight_bytes: n * BLK + fixed_bytes,
            read_bytes_per_token: n * BLK + fixed_read,
            flops_per_token: n * BLK_FLOPS + fixed_flops,
            kv_bytes_per_token: n * KV,
        }
    }
    fn perf() -> Vec<DevicePerf> {
        vec![
            DevicePerf {
                device_id: "laptop".into(),
                bw_bytes_per_s: 14.3e9,
                bw_src: Src::Derived,
                gflops: 122.0,
                gflops_src: Src::Derived,
            },
            DevicePerf {
                device_id: "phone".into(),
                bw_bytes_per_s: 18.2e9,
                bw_src: Src::Derived,
                gflops: 81.0,
                gflops_src: Src::Derived,
            },
        ]
    }
    fn adb() -> Vec<LinkInput> {
        vec![LinkInput {
            device_id: "phone".into(),
            alpha_ms: 717.0,
            alpha_src: Src::Derived,
            beta_bytes_per_s: 20e6,
            beta_src: Src::Estimate,
        }]
    }

    #[test]
    fn laptop_alone_reproduces_the_measured_three_tokens_per_second() {
        let req = Request::default();
        let p = predict(&req, &[stack("laptop", true, 0, 36)], &perf(), &[]);
        // ≈ (36 × 115.8 MB + 510 MB + KV) / 14.3 GB/s ≈ 334 ms → ≈ 3.0 tok/s
        assert!(
            (p.token_ms.v - 334.0).abs() < 25.0,
            "token_ms {}",
            p.token_ms.v
        );
        assert!((p.tok_s.v - 3.0).abs() < 0.3, "tok/s {}", p.tok_s.v);
        // first word for a 500-token prompt ≈ 58 s (compute-bound prefill)
        assert!(
            p.ttft_ms.v > 40_000.0 && p.ttft_ms.v < 90_000.0,
            "ttft {}",
            p.ttft_ms.v
        );
        assert_eq!(p.tok_s.src, Src::Derived);
        assert!(p.bottleneck.contains("laptop"));
    }

    #[test]
    fn adb_split_is_link_bound_at_about_one_token_per_second() {
        let req = Request::default();
        let p = predict(
            &req,
            &[stack("laptop", true, 0, 23), stack("phone", false, 23, 36)],
            &perf(),
            &adb(),
        );
        assert!((p.tok_s.v - 1.0).abs() < 0.15, "tok/s {}", p.tok_s.v);
        assert!(
            p.bottleneck.starts_with("link to phone"),
            "{}",
            p.bottleneck
        );
        // the two stacks' compute alone is faster than the laptop alone: splitting is for memory
        let compute: f64 = p.stacks.iter().map(|s| s.decode_ms.v).sum();
        assert!(compute < 334.0);
        assert_eq!(p.links.len(), 1);
    }

    #[test]
    fn tethering_estimate_ties_with_the_laptop_alone() {
        let req = Request::default();
        let links = vec![LinkInput {
            device_id: "phone".into(),
            alpha_ms: 7.0,
            alpha_src: Src::Estimate,
            beta_bytes_per_s: 100e6,
            beta_src: Src::Estimate,
        }];
        let p = predict(
            &req,
            &[stack("laptop", true, 0, 23), stack("phone", false, 23, 36)],
            &perf(),
            &links,
        );
        assert!((p.tok_s.v - 3.1).abs() < 0.4, "tok/s {}", p.tok_s.v);
        assert_eq!(p.tok_s.src, Src::Estimate); // weakest input wins
    }

    #[test]
    fn missing_measurements_yield_unknown_not_a_number() {
        let p = predict(
            &Request::default(),
            &[stack("laptop", true, 0, 36)],
            &[],
            &[],
        );
        assert_eq!(p.tok_s.src, Src::Unknown);
        assert!(!p.tok_s.is_known());
        assert!(p
            .warnings
            .iter()
            .any(|w| w.contains("no speed measurement")));
        let p = predict(
            &Request::default(),
            &[stack("laptop", true, 0, 23), stack("phone", false, 23, 36)],
            &perf(),
            &[],
        );
        assert_eq!(p.tok_s.src, Src::Unknown);
    }

    #[test]
    fn calibration_helpers_round_trip() {
        // 3.05 tok/s on the laptop-alone stack → BW ≈ 14.3 GB/s
        let s = stack("laptop", true, 0, 36);
        let bw = bandwidth_from_run(s.read_bytes_per_token, 600 * s.kv_bytes_per_token, 3.05);
        assert!((bw - 14.3e9).abs() / 14.3e9 < 0.05, "bw {bw}");
        let g = gflops_from_run(s.flops_per_token, 8.76);
        assert!(g > 100.0 && g < 170.0, "gflops {g}");
        assert_eq!(
            alpha_from_split(755.0, 38.0),
            Some(755.0 - 38.0 - OVERHEAD_MS)
        );
        assert_eq!(alpha_from_split(30.0, 38.0), None);
        assert_eq!(Src::Measured.weakest(Src::Estimate), Src::Estimate);
        assert!(Est::new(100.0, Src::Measured).hi > 100.0);
    }
}
