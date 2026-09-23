//! Minimal GGUF reader: header, key/value metadata, tensor table.
//!
//! Per-tensor byte sizes are derived from the *offsets* of consecutive tensors in the data
//! section (plus the file length for the last one), so no ggml quant-type table is needed and
//! new quant formats (MXFP4, IQ*) are handled automatically. Sizes therefore include the
//! ≤32-byte alignment padding, which is irrelevant at the MB scale we plan with.
//!
//! Spec: https://github.com/ggml-org/ggml/blob/master/docs/gguf.md

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::Path;

const MAGIC: &[u8; 4] = b"GGUF";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    Bool(bool),
    Str(String),
    U64(u64),
    I64(i64),
    F64(f64),
    /// Arrays are kept only as their length + element type to avoid pulling 150k-token vocabularies into memory.
    Array {
        elem_type: u32,
        len: u64,
    },
}

impl Value {
    pub fn as_u64(&self) -> Option<u64> {
        match *self {
            Value::U8(v) => Some(v as u64),
            Value::U16(v) => Some(v as u64),
            Value::U32(v) => Some(v as u64),
            Value::U64(v) => Some(v),
            Value::I8(v) if v >= 0 => Some(v as u64),
            Value::I16(v) if v >= 0 => Some(v as u64),
            Value::I32(v) if v >= 0 => Some(v as u64),
            Value::I64(v) if v >= 0 => Some(v as u64),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Value::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorInfo {
    pub name: String,
    pub dims: Vec<u64>,
    pub ggml_type: u32,
    pub offset: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub path: String,
    pub file_bytes: u64,
    pub version: u32,
    pub arch: String,
    pub name: String,
    pub quant_label: String,
    pub n_layer: u32,
    pub n_embd: u32,
    pub n_head: u32,
    pub n_head_kv: u32,
    pub n_ctx_train: u32,
    pub n_expert: u32,
    pub n_expert_used: u32,
    /// Bytes of tensors that live outside any `blk.N.` group (token embeddings, output head, norms).
    pub non_layer_bytes: u64,
    /// Bytes per transformer block, index = layer.
    pub layer_bytes: Vec<u64>,
    /// Approximate KV-cache bytes per context token at f16 (both K and V, all layers).
    pub kv_bytes_per_token: u64,
    pub tensor_count: u64,
}

impl ModelInfo {
    pub fn weight_bytes(&self) -> u64 {
        self.non_layer_bytes + self.layer_bytes.iter().sum::<u64>()
    }
    pub fn kv_bytes(&self, n_ctx: u32) -> u64 {
        self.kv_bytes_per_token * n_ctx as u64
    }
    pub fn is_moe(&self) -> bool {
        self.n_expert > 1
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GgufError {
    #[error("not a GGUF file (bad magic)")]
    BadMagic,
    #[error("unsupported GGUF version {0}")]
    Version(u32),
    #[error("unknown value type {0}")]
    ValueType(u32),
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("malformed: {0}")]
    Malformed(&'static str),
}

struct Rd<R: Read> {
    r: R,
}
impl<R: Read> Rd<R> {
    fn u8(&mut self) -> io::Result<u8> {
        let mut b = [0u8; 1];
        self.r.read_exact(&mut b)?;
        Ok(b[0])
    }
    fn u16(&mut self) -> io::Result<u16> {
        let mut b = [0u8; 2];
        self.r.read_exact(&mut b)?;
        Ok(u16::from_le_bytes(b))
    }
    fn u32(&mut self) -> io::Result<u32> {
        let mut b = [0u8; 4];
        self.r.read_exact(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }
    fn u64(&mut self) -> io::Result<u64> {
        let mut b = [0u8; 8];
        self.r.read_exact(&mut b)?;
        Ok(u64::from_le_bytes(b))
    }
    fn f32(&mut self) -> io::Result<f32> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn f64(&mut self) -> io::Result<f64> {
        Ok(f64::from_bits(self.u64()?))
    }
    fn string(&mut self) -> Result<String, GgufError> {
        let n = self.u64()?;
        if n > 1 << 24 {
            return Err(GgufError::Malformed("string too long"));
        }
        let mut b = vec![0u8; n as usize];
        self.r.read_exact(&mut b)?;
        Ok(String::from_utf8_lossy(&b).into_owned())
    }
    fn skip(&mut self, n: u64) -> io::Result<()> {
        io::copy(&mut (&mut self.r).take(n), &mut io::sink())?;
        Ok(())
    }
    fn scalar_size(t: u32) -> Option<u64> {
        Some(match t {
            0 | 1 | 7 => 1,
            2 | 3 => 2,
            4 | 5 | 6 => 4,
            10 | 11 | 12 => 8,
            _ => return None,
        })
    }
    fn value(&mut self, t: u32) -> Result<Value, GgufError> {
        Ok(match t {
            0 => Value::U8(self.u8()?),
            1 => Value::I8(self.u8()? as i8),
            2 => Value::U16(self.u16()?),
            3 => Value::I16(self.u16()? as i16),
            4 => Value::U32(self.u32()?),
            5 => Value::I32(self.u32()? as i32),
            6 => Value::F32(self.f32()?),
            7 => Value::Bool(self.u8()? != 0),
            8 => Value::Str(self.string()?),
            9 => {
                let et = self.u32()?;
                let len = self.u64()?;
                if et == 8 {
                    for _ in 0..len {
                        let n = self.u64()?;
                        self.skip(n)?;
                    }
                } else if et == 9 {
                    return Err(GgufError::Malformed("nested arrays unsupported"));
                } else {
                    let sz = Self::scalar_size(et).ok_or(GgufError::ValueType(et))?;
                    self.skip(sz * len)?;
                }
                Value::Array { elem_type: et, len }
            }
            10 => Value::U64(self.u64()?),
            11 => Value::I64(self.u64()? as i64),
            12 => Value::F64(self.f64()?),
            other => return Err(GgufError::ValueType(other)),
        })
    }
}

/// Read a GGUF file's metadata and tensor table. Touches only the header, never the weights.
pub fn read(path: impl AsRef<Path>) -> Result<ModelInfo, GgufError> {
    let path = path.as_ref();
    let file_bytes = std::fs::metadata(path)?.len();
    let mut f = BufReader::with_capacity(1 << 20, File::open(path)?);
    let mut rd = Rd { r: &mut f };

    let mut magic = [0u8; 4];
    rd.r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(GgufError::BadMagic);
    }
    let version = rd.u32()?;
    if !(2..=3).contains(&version) {
        return Err(GgufError::Version(version));
    }
    let n_tensors = rd.u64()?;
    let n_kv = rd.u64()?;

    let mut kv: BTreeMap<String, Value> = BTreeMap::new();
    for _ in 0..n_kv {
        let key = rd.string()?;
        let t = rd.u32()?;
        let v = rd.value(t)?;
        kv.insert(key, v);
    }

    let mut tensors: Vec<TensorInfo> = Vec::with_capacity(n_tensors.min(1 << 16) as usize);
    for _ in 0..n_tensors {
        let name = rd.string()?;
        let n_dims = rd.u32()?;
        if n_dims > 8 {
            return Err(GgufError::Malformed("n_dims > 8"));
        }
        let mut dims = Vec::with_capacity(n_dims as usize);
        for _ in 0..n_dims {
            dims.push(rd.u64()?);
        }
        let ggml_type = rd.u32()?;
        let offset = rd.u64()?;
        tensors.push(TensorInfo {
            name,
            dims,
            ggml_type,
            offset,
            bytes: 0,
        });
    }

    // Data section starts at the header end aligned to `general.alignment` (default 32).
    let header_end = f.stream_position()?;
    let alignment = kv
        .get("general.alignment")
        .and_then(Value::as_u64)
        .unwrap_or(32)
        .max(1);
    let data_start = header_end.div_ceil(alignment) * alignment;
    let data_len = file_bytes.saturating_sub(data_start);

    // Byte size of each tensor = distance to the next tensor's offset (offsets are relative to data_start).
    let mut order: Vec<usize> = (0..tensors.len()).collect();
    order.sort_by_key(|&i| tensors[i].offset);
    for w in 0..order.len() {
        let i = order[w];
        let end = if w + 1 < order.len() {
            tensors[order[w + 1]].offset
        } else {
            data_len
        };
        tensors[i].bytes = end.saturating_sub(tensors[i].offset);
    }
    f.seek(SeekFrom::Start(0))?;

    let arch = kv
        .get("general.architecture")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let get_u32 = |k: &str| {
        kv.get(&format!("{arch}.{k}"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32
    };
    let n_layer = get_u32("block_count");
    let n_embd = get_u32("embedding_length");
    let n_head = get_u32("attention.head_count");
    let n_head_kv = {
        let v = get_u32("attention.head_count_kv");
        if v == 0 {
            n_head
        } else {
            v
        }
    };
    let n_ctx_train = get_u32("context_length");
    let n_expert = get_u32("expert_count");
    let n_expert_used = get_u32("expert_used_count");
    let head_dim_k = {
        let v = get_u32("attention.key_length");
        if v == 0 && n_head > 0 {
            n_embd / n_head
        } else {
            v
        }
    };
    let head_dim_v = {
        let v = get_u32("attention.value_length");
        if v == 0 {
            head_dim_k
        } else {
            v
        }
    };
    // f16 K and V per layer per token.
    let kv_bytes_per_token =
        n_layer as u64 * n_head_kv as u64 * (head_dim_k as u64 + head_dim_v as u64) * 2;

    let mut layer_bytes = vec![0u64; n_layer as usize];
    let mut non_layer_bytes = 0u64;
    for t in &tensors {
        match layer_index(&t.name) {
            Some(i) if (i as usize) < layer_bytes.len() => layer_bytes[i as usize] += t.bytes,
            _ => non_layer_bytes += t.bytes,
        }
    }

    let name = kv
        .get("general.name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let quant_label = quant_label_from(path, &kv);

    Ok(ModelInfo {
        path: path.display().to_string(),
        file_bytes,
        version,
        arch,
        name,
        quant_label,
        n_layer,
        n_embd,
        n_head,
        n_head_kv,
        n_ctx_train,
        n_expert,
        n_expert_used,
        non_layer_bytes,
        layer_bytes,
        kv_bytes_per_token,
        tensor_count: n_tensors,
    })
}

/// `blk.17.attn_q.weight` → Some(17)
fn layer_index(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("blk.")?;
    let end = rest.find('.')?;
    rest[..end].parse().ok()
}

fn quant_label_from(path: &Path, kv: &BTreeMap<String, Value>) -> String {
    // Prefer the file-name convention (Q4_K_M, MXFP4, Q8_0…), fall back to general.file_type.
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    for tok in stem.rsplit(['-', '_', '.']).take(3) {
        let up = tok.to_ascii_uppercase();
        if up.starts_with('Q') && up.len() >= 2 && up.as_bytes()[1].is_ascii_digit() {
            // Re-join things like Q4_K_M which split on '_'
            if let Some(pos) = stem.to_ascii_uppercase().find(&up) {
                return stem[pos..].to_string();
            }
        }
        if up == "MXFP4" || up.starts_with("IQ") || up == "BF16" || up == "F16" {
            return up;
        }
    }
    match kv.get("general.file_type").and_then(Value::as_u64) {
        Some(1) => "F16".into(),
        Some(2) => "Q4_0".into(),
        Some(7) => "Q8_0".into(),
        Some(15) => "Q4_K_M".into(),
        Some(t) => format!("ftype{t}"),
        None => "unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_index_parses() {
        assert_eq!(layer_index("blk.17.attn_q.weight"), Some(17));
        assert_eq!(layer_index("token_embd.weight"), None);
        assert_eq!(layer_index("blk.x.y"), None);
    }

    #[test]
    fn real_model_if_present() {
        // Runs only when the test model exists (CI without the file still passes).
        let p =
            std::env::var("MESHAI_MODELS").unwrap_or_else(|_| "/mnt/storage/meshai/models".into());
        let f = std::path::Path::new(&p).join("Qwen3-0.6B-Q8_0.gguf");
        if !f.exists() {
            eprintln!("skip: {} missing", f.display());
            return;
        }
        let m = read(&f).unwrap();
        assert_eq!(m.arch, "qwen3");
        assert_eq!(m.n_layer, 28);
        assert_eq!(m.layer_bytes.len(), 28);
        assert!(m.layer_bytes.iter().all(|&b| b > 0));
        // weights must account for (almost) the whole file
        let acct = m.weight_bytes() as f64 / m.file_bytes as f64;
        assert!(acct > 0.95 && acct <= 1.0, "accounted {acct}");
        assert!(m.kv_bytes_per_token > 0);
    }
}
