//! Layer config: the placement units of a GGUF model and the rules for combining them
//! (docs/MESHAI.md §17.3, D032/D033, task T072).
//!
//! Units, in file order:
//! * `embd`: `token_embd.weight` (+ its norm if any). Pinned to the host: llama.cpp always keeps
//!   the input embedding on the CPU.
//! * `blk.i`: everything named `blk.i.*` (attention, dense FFN, router + experts) plus layer i's
//!   KV cache at run time. MUST_STAY_TOGETHER (attention/FFN by policy, all experts of a layer
//!   and its KV by the engine); blocks form contiguous ranges per device, host prefix first
//!   (`-ngl` / `--tensor-split` semantics).
//! * `head`: `output_norm.*` + `output.*` (tied models: the `token_embd` duplicate, counted once).
//!   Pinned to the host by policy (`-ot ^(output|output_norm|token_embd)\.(weight|bias)$=CPU`).
//! * `other`: anything else outside a block (rope frequencies, cls heads…): counted on the host.
//!
//! Numbers are exact bytes from the tensor table; FLOPs are the usual 2 × params per token for
//! 2-D/3-D matmul weights (norms and biases are not counted); MoE read-bytes assume only
//! `n_expert_used` of `n_expert` experts are touched per token (estimate, K25).

use crate::gguf::{layer_index_of, Gguf, TensorInfo};
use serde::{Deserialize, Serialize};

/// What the engine or our policy says about where a unit may go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Rule {
    /// Never leaves the device that runs the engine.
    PinnedToHost,
    /// The unit is indivisible: all its tensors (and its KV cache) live on one device.
    MustStayTogether,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitKind {
    TokenEmbedding,
    Block,
    Output,
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct Unit {
    pub id: String,
    pub kind: UnitKind,
    /// Block index for `Block`, otherwise `None`.
    pub index: Option<u32>,
    /// Bytes of weights in the file (exact).
    pub bytes: u64,
    /// Weight parameters (product of dims of every tensor in the unit).
    pub params: u64,
    /// Bytes read from memory per decoded token: dense weights + `n_expert_used/n_expert` of the
    /// experts (estimate for MoE, exact for dense).
    pub read_bytes_per_token: u64,
    /// 2 × matmul params per token, experts scaled by `n_expert_used/n_expert`.
    pub flops_per_token: u64,
    /// KV-cache bytes per context token for this block (0 for non-blocks).
    pub kv_bytes_per_token: u64,
    pub attn_bytes: u64,
    pub ffn_bytes: u64,
    pub expert_bytes: u64,
    pub router_bytes: u64,
    pub rule: Rule,
    /// "engine" (a llama.cpp fact) or "policy" (a MeshAI choice).
    pub source: &'static str,
    pub tensors: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Constraint {
    pub id: &'static str,
    pub rule: &'static str,
    pub what: &'static str,
    pub source: &'static str,
    pub enforced_by: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shape {
    pub arch: String,
    pub n_layer: u32,
    pub n_embd: u32,
    pub n_head: u32,
    pub n_head_kv: u32,
    pub n_vocab: u64,
    pub n_ctx_train: u32,
    pub n_expert: u32,
    pub n_expert_used: u32,
    pub tied_embeddings: bool,
    /// `{arch}.attention.sliding_window` when the file declares one (Gemma 3, gpt-oss): KV of the
    /// windowed layers is capped at this many tokens. Which layers are windowed is not decoded here.
    pub sliding_window: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayerConfig {
    pub model: String,
    pub file_bytes: u64,
    pub quant: String,
    pub shape: Shape,
    /// Activation crossing a device boundary per token: n_embd × 4 bytes (f32).
    pub activation_bytes: u64,
    /// KV bytes per context token, all layers (f16 K+V).
    pub kv_bytes_per_token: u64,
    pub units: Vec<Unit>,
    pub constraints: Vec<Constraint>,
    pub warnings: Vec<String>,
}

impl LayerConfig {
    pub fn blocks(&self) -> impl Iterator<Item = &Unit> {
        self.units.iter().filter(|u| u.kind == UnitKind::Block)
    }
    /// Bytes the host must hold regardless of the split (embedding + head + other).
    pub fn host_fixed_bytes(&self) -> u64 {
        self.units
            .iter()
            .filter(|u| u.kind != UnitKind::Block)
            .map(|u| u.bytes)
            .sum()
    }
    pub fn total_bytes(&self) -> u64 {
        self.units.iter().map(|u| u.bytes).sum()
    }
    /// Blocks `a..b` (half-open) as one stack: bytes, read bytes and FLOPs per token, KV per token.
    pub fn stack(&self, a: u32, b: u32) -> (u64, u64, u64, u64) {
        let mut w = (0u64, 0u64, 0u64, 0u64);
        for u in self.blocks() {
            if let Some(i) = u.index {
                if i >= a && i < b {
                    w.0 += u.bytes;
                    w.1 += u.read_bytes_per_token;
                    w.2 += u.flops_per_token;
                    w.3 += u.kv_bytes_per_token;
                }
            }
        }
        w
    }
}

fn params_of(t: &TensorInfo) -> u64 {
    t.dims.iter().product::<u64>()
}

fn is_matmul(t: &TensorInfo) -> bool {
    t.dims.len() >= 2 && !t.name.ends_with("norm.weight") && !t.name.ends_with(".bias")
}

/// Build the layer config from a fully read GGUF header.
pub fn layer_config(g: &Gguf) -> LayerConfig {
    let info = &g.info;
    let n_layer = info.n_layer;
    let n_expert = info.n_expert.max(1) as u64;
    let n_used = if info.n_expert > 1 {
        info.n_expert_used.clamp(1, info.n_expert) as u64
    } else {
        1
    };
    let kv_per_layer = if n_layer > 0 {
        info.kv_bytes_per_token / n_layer as u64
    } else {
        0
    };
    let mut warnings = Vec::new();

    // Per-block accumulators.
    #[derive(Default, Clone)]
    struct Acc {
        bytes: u64,
        params: u64,
        read: u64,
        flops: u64,
        attn: u64,
        ffn: u64,
        experts: u64,
        router: u64,
        tensors: u32,
    }
    let mut blocks = vec![Acc::default(); n_layer as usize];
    let mut embd = Acc::default();
    let mut head = Acc::default();
    let mut other = Acc::default();
    let mut has_output_weight = false;
    let mut n_vocab = 0u64;

    for t in &g.tensors {
        let p = params_of(t);
        let mm = is_matmul(t);
        match layer_index_of(&t.name) {
            Some(i) if (i as usize) < blocks.len() => {
                let b = &mut blocks[i as usize];
                b.bytes += t.bytes;
                b.params += p;
                b.tensors += 1;
                let short = t.name.splitn(3, '.').nth(2).unwrap_or("");
                let is_expert = short.contains("_exps");
                let is_router = short.starts_with("ffn_gate_inp");
                if is_expert {
                    b.experts += t.bytes;
                    b.read += t.bytes * n_used / n_expert;
                    if mm {
                        b.flops += 2 * p * n_used / n_expert;
                    }
                } else {
                    b.read += t.bytes;
                    if mm {
                        b.flops += 2 * p;
                    }
                    if is_router {
                        b.router += t.bytes;
                    } else if short.starts_with("attn") {
                        b.attn += t.bytes;
                    } else {
                        b.ffn += t.bytes;
                    }
                }
            }
            Some(_) => {
                warnings.push(format!("tensor {} names a block beyond n_layer", t.name));
                other.bytes += t.bytes;
                other.params += p;
                other.tensors += 1;
            }
            None => {
                let acc = if t.name.starts_with("token_embd") {
                    if t.name == "token_embd.weight" {
                        n_vocab = t.dims.get(1).copied().unwrap_or(0);
                    }
                    &mut embd
                } else if t.name.starts_with("output") {
                    if t.name == "output.weight" {
                        has_output_weight = true;
                    }
                    &mut head
                } else {
                    &mut other
                };
                acc.bytes += t.bytes;
                acc.params += p;
                acc.tensors += 1;
                acc.read += t.bytes;
                if mm && !t.name.starts_with("token_embd") {
                    acc.flops += 2 * p;
                }
            }
        }
    }
    let tied = !has_output_weight;
    if tied {
        // The head is a duplicate of token_embd: its matmul still costs 2 × params per token,
        // and the pinned copy may be a second allocation on the host (R010, unverified).
        head.flops += 2 * embd.params;
        head.read += embd.bytes;
        warnings.push(
            "tied embeddings: the output head reuses token_embd (bytes counted once; a pinned copy may cost a second allocation, R010)"
                .into(),
        );
    }
    if other.bytes > 0 {
        warnings.push(format!(
            "{} bytes in {} tensors outside blocks/embedding/head are counted on the host",
            other.bytes, other.tensors
        ));
    }

    let mut units = Vec::with_capacity(n_layer as usize + 3);
    units.push(Unit {
        id: "embd".into(),
        kind: UnitKind::TokenEmbedding,
        index: None,
        bytes: embd.bytes,
        params: embd.params,
        read_bytes_per_token: (info.n_embd as u64) * 4, // one row lookup, not the whole table
        flops_per_token: 0,
        kv_bytes_per_token: 0,
        attn_bytes: 0,
        ffn_bytes: 0,
        expert_bytes: 0,
        router_bytes: 0,
        rule: Rule::PinnedToHost,
        source: "engine",
        tensors: embd.tensors,
    });
    for (i, b) in blocks.iter().enumerate() {
        units.push(Unit {
            id: format!("blk.{i}"),
            kind: UnitKind::Block,
            index: Some(i as u32),
            bytes: b.bytes,
            params: b.params,
            read_bytes_per_token: b.read,
            flops_per_token: b.flops,
            kv_bytes_per_token: kv_per_layer,
            attn_bytes: b.attn,
            ffn_bytes: b.ffn,
            expert_bytes: b.experts,
            router_bytes: b.router,
            rule: Rule::MustStayTogether,
            source: if b.experts > 0 { "engine" } else { "policy" },
            tensors: b.tensors,
        });
    }
    units.push(Unit {
        id: "head".into(),
        kind: UnitKind::Output,
        index: None,
        bytes: head.bytes,
        params: head.params,
        read_bytes_per_token: head.read,
        flops_per_token: head.flops,
        kv_bytes_per_token: 0,
        attn_bytes: 0,
        ffn_bytes: 0,
        expert_bytes: 0,
        router_bytes: 0,
        rule: Rule::PinnedToHost,
        source: "policy",
        tensors: head.tensors,
    });
    if other.bytes > 0 {
        units.push(Unit {
            id: "other".into(),
            kind: UnitKind::Other,
            index: None,
            bytes: other.bytes,
            params: other.params,
            read_bytes_per_token: other.read,
            flops_per_token: other.flops,
            kv_bytes_per_token: 0,
            attn_bytes: 0,
            ffn_bytes: 0,
            expert_bytes: 0,
            router_bytes: 0,
            rule: Rule::PinnedToHost,
            source: "policy",
            tensors: other.tensors,
        });
    }

    let sliding_window =
        g.kv.get(&format!("{}.attention.sliding_window", info.arch))
            .and_then(|v| v.as_u64())
            .filter(|&w| w > 0);

    let constraints = vec![
        Constraint {
            id: "C1",
            rule: "MUST_STAY_TOGETHER",
            what: "a block's attention, FFN/experts and its KV cache",
            source: "policy+engine",
            enforced_by: "whole layers via -ngl / --tensor-split",
        },
        Constraint {
            id: "C2",
            rule: "MAY_SPLIT_CONTIGUOUS",
            what: "one block range per device, host prefix first, in --rpc order",
            source: "engine interface + policy",
            enforced_by: "llama-model.cpp layer assignment",
        },
        Constraint {
            id: "C3",
            rule: "PINNED_TO_HOST",
            what: "token embedding",
            source: "engine",
            enforced_by: "llama.cpp keeps token_embd on the CPU",
        },
        Constraint {
            id: "C4",
            rule: "PINNED_TO_HOST",
            what: "output norm + head (incl. the tied token_embd duplicate)",
            source: "policy",
            enforced_by: "-ot ^(output|output_norm|token_embd)\\.(weight|bias)$=CPU",
        },
        Constraint {
            id: "C5",
            rule: "PINNED_TO_HOST",
            what: "vision encoder + projector (separate mmproj file)",
            source: "policy",
            enforced_by: "--no-mmproj-offload",
        },
        Constraint {
            id: "C6",
            rule: "MAX_RPC_DEVICES",
            what: "at most 16 RPC workers; worker-to-worker traffic goes through the host",
            source: "engine",
            enforced_by: "ggml-rpc",
        },
    ];

    LayerConfig {
        model: std::path::Path::new(&info.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(&info.path)
            .to_string(),
        file_bytes: info.file_bytes,
        quant: info.quant_label.clone(),
        shape: Shape {
            arch: info.arch.clone(),
            n_layer,
            n_embd: info.n_embd,
            n_head: info.n_head,
            n_head_kv: info.n_head_kv,
            n_vocab,
            n_ctx_train: info.n_ctx_train,
            n_expert: info.n_expert,
            n_expert_used: info.n_expert_used,
            tied_embeddings: tied,
            sliding_window,
        },
        activation_bytes: info.n_embd as u64 * 4,
        kv_bytes_per_token: info.kv_bytes_per_token,
        units,
        constraints,
        warnings,
    }
}

/// Plain-text summary for `meshd layers <file>`.
pub fn summary(c: &LayerConfig) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "{} · {} · {} layers · n_embd {} · {} experts ({} used) · tied {} · {:.2} GB\n",
        c.model,
        c.quant,
        c.shape.n_layer,
        c.shape.n_embd,
        c.shape.n_expert,
        c.shape.n_expert_used,
        c.shape.tied_embeddings,
        c.file_bytes as f64 / 1e9
    ));
    for u in &c.units {
        if u.kind == UnitKind::Block {
            continue;
        }
        s.push_str(&format!(
            "  {:<6} {:>8.1} MB  {:?}  ({})\n",
            u.id,
            u.bytes as f64 / 1e6,
            u.rule,
            u.source
        ));
    }
    let (b, r, f, kv) = c.stack(0, c.shape.n_layer);
    let n = c.shape.n_layer.max(1) as f64;
    s.push_str(&format!(
        "  blocks {:>8.1} MB total · {:.1} MB per block · {:.1} MB read/token/block · {:.2} GFLOP/token/block · {} B KV/token/block\n",
        b as f64 / 1e6,
        b as f64 / 1e6 / n,
        r as f64 / 1e6 / n,
        f as f64 / 1e9 / n,
        kv / c.shape.n_layer.max(1) as u64
    ));
    s.push_str(&format!(
        "  host fixed {:.1} MB · activation/token {} B · KV/token all layers {} B\n",
        c.host_fixed_bytes() as f64 / 1e6,
        c.activation_bytes,
        c.kv_bytes_per_token
    ));
    for w in &c.warnings {
        s.push_str(&format!("  ! {w}\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gguf::ModelInfo;
    use std::collections::BTreeMap;

    fn t(name: &str, dims: &[u64], bytes: u64) -> TensorInfo {
        TensorInfo {
            name: name.into(),
            dims: dims.to_vec(),
            ggml_type: 0,
            offset: 0,
            bytes,
        }
    }

    fn fake(n_layer: u32, tied: bool, moe: bool) -> Gguf {
        let mut tensors = vec![t("token_embd.weight", &[64, 1000], 64_000)];
        for i in 0..n_layer {
            tensors.push(t(&format!("blk.{i}.attn_norm.weight"), &[64], 64));
            tensors.push(t(&format!("blk.{i}.attn_q.weight"), &[64, 64], 4_096));
            tensors.push(t(&format!("blk.{i}.attn_output.weight"), &[64, 64], 4_096));
            if moe {
                tensors.push(t(&format!("blk.{i}.ffn_gate_inp.weight"), &[64, 4], 256));
                tensors.push(t(
                    &format!("blk.{i}.ffn_up_exps.weight"),
                    &[64, 128, 4],
                    32_768,
                ));
                tensors.push(t(
                    &format!("blk.{i}.ffn_down_exps.weight"),
                    &[128, 64, 4],
                    32_768,
                ));
            } else {
                tensors.push(t(&format!("blk.{i}.ffn_up.weight"), &[64, 128], 8_192));
                tensors.push(t(&format!("blk.{i}.ffn_down.weight"), &[128, 64], 8_192));
            }
        }
        tensors.push(t("output_norm.weight", &[64], 64));
        if !tied {
            tensors.push(t("output.weight", &[64, 1000], 64_000));
        }
        let non_layer: u64 = tensors
            .iter()
            .filter(|x| layer_index_of(&x.name).is_none())
            .map(|x| x.bytes)
            .sum();
        let mut layer_bytes = vec![0u64; n_layer as usize];
        for x in &tensors {
            if let Some(i) = layer_index_of(&x.name) {
                layer_bytes[i as usize] += x.bytes;
            }
        }
        Gguf {
            info: ModelInfo {
                path: "/models/fake.gguf".into(),
                file_bytes: tensors.iter().map(|x| x.bytes).sum(),
                version: 3,
                arch: "test".into(),
                name: "fake".into(),
                quant_label: "Q8_0".into(),
                n_layer,
                n_embd: 64,
                n_head: 4,
                n_head_kv: 2,
                n_ctx_train: 4096,
                n_expert: if moe { 4 } else { 0 },
                n_expert_used: if moe { 1 } else { 0 },
                non_layer_bytes: non_layer,
                layer_bytes,
                kv_bytes_per_token: n_layer as u64 * 2 * (16 + 16) * 2,
                tensor_count: tensors.len() as u64,
            },
            kv: BTreeMap::new(),
            tensors,
        }
    }

    #[test]
    fn units_cover_every_byte_of_the_file() {
        let g = fake(4, false, false);
        let c = layer_config(&g);
        assert_eq!(c.total_bytes(), g.info.file_bytes);
        assert_eq!(c.blocks().count(), 4);
        assert_eq!(c.host_fixed_bytes(), 64_000 + 64 + 64_000);
        assert!(!c.shape.tied_embeddings);
        assert_eq!(c.shape.n_vocab, 1000);
        let b0 = c.blocks().next().unwrap();
        assert_eq!(b0.attn_bytes, 64 + 4_096 + 4_096);
        assert_eq!(b0.ffn_bytes, 8_192 * 2);
        assert_eq!(b0.read_bytes_per_token, b0.bytes);
        // 2 × (64·64 + 64·64 + 64·128 + 128·64) matmul params; the norm is not counted
        assert_eq!(b0.flops_per_token, 2 * (4096 + 4096 + 8192 + 8192));
        assert_eq!(b0.kv_bytes_per_token, 2 * 32 * 2);
        assert_eq!(b0.rule, Rule::MustStayTogether);
        assert_eq!(c.units.first().unwrap().rule, Rule::PinnedToHost);
        assert_eq!(c.units.last().unwrap().kind, UnitKind::Output);
    }

    #[test]
    fn tied_embeddings_are_counted_once_but_the_head_still_costs_flops() {
        let c = layer_config(&fake(2, true, false));
        assert!(c.shape.tied_embeddings);
        let head = c.units.iter().find(|u| u.kind == UnitKind::Output).unwrap();
        assert_eq!(head.bytes, 64); // only the norm lives in the file
        assert_eq!(head.flops_per_token, 2 * 64 * 1000);
        assert!(c.warnings.iter().any(|w| w.contains("tied")));
    }

    #[test]
    fn moe_reads_only_the_active_experts() {
        let c = layer_config(&fake(2, false, true));
        let b = c.blocks().next().unwrap();
        assert_eq!(b.expert_bytes, 32_768 * 2);
        assert_eq!(b.router_bytes, 256);
        // dense part fully read, experts scaled by 1/4
        let dense = 64 + 4_096 + 4_096 + 256;
        assert_eq!(b.read_bytes_per_token, dense + 32_768 * 2 / 4);
        assert_eq!(b.source, "engine");
        let (bytes, read, _, _) = c.stack(0, 2);
        assert_eq!(bytes, 2 * b.bytes);
        assert_eq!(read, 2 * b.read_bytes_per_token);
    }

    #[test]
    fn real_model_if_present() {
        let p =
            std::env::var("MESHAI_MODELS").unwrap_or_else(|_| "/mnt/storage/meshai/models".into());
        let f = std::path::Path::new(&p).join("Qwen3-0.6B-Q8_0.gguf");
        if !f.exists() {
            eprintln!("skip: {} missing", f.display());
            return;
        }
        let g = crate::gguf::read_full(&f).unwrap();
        let c = layer_config(&g);
        assert_eq!(c.blocks().count(), 28);
        // Every byte of the tensor data is in exactly one unit; the file is larger only by its
        // header (metadata + tokenizer, ≈6 MB for a Qwen vocabulary) and alignment padding.
        let diff = g.info.file_bytes.abs_diff(c.total_bytes());
        assert!(
            c.total_bytes() <= g.info.file_bytes && diff < g.info.file_bytes / 100,
            "units vs file differ by {diff} bytes"
        );
        assert!(c.shape.tied_embeddings, "Qwen3-0.6B ties its embeddings");
        assert_eq!(c.shape.n_vocab, 151_936);
    }
}
