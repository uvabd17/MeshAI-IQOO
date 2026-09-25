//! Model catalog: known demo models, HTTP downloads with resume + size check, file serving for phones.

use crate::state::{now_ms, AppState, Download};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// The reference model every phone benches at join time (control.rs) and the layer config the
/// Calculate step uses to turn that bench into a bandwidth figure (calc.rs).
pub const BENCH_MODEL_FILE: &str = "Qwen3-0.6B-Q8_0.gguf";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub provider: String, // qwen | openai | anthropic | huggingface
    pub name: String,
    pub file: String,
    pub url: String,
    pub bytes: u64,
    pub layers: u32,
    pub role: String, // baseline | budget | headline | test | cloud | vision | projector
    pub note: String,
    /// Multimodal projector file that must sit next to the model for image input (llama-server --mmproj).
    #[serde(default)]
    pub mmproj: Option<String>,
}

/// The demo set (D012). `bytes` is the model-card size; the download checks the served length.
pub fn catalog() -> Vec<CatalogEntry> {
    vec![
        CatalogEntry { id: "qwen3-0.6b".into(), provider: "qwen".into(), name: "Qwen3 0.6B".into(), file: "Qwen3-0.6B-Q8_0.gguf".into(), url: "https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf".into(), bytes: 639_446_688, layers: 28, role: "test".into(), note: "Split tests and CI".into() , mmproj: None },
        CatalogEntry { id: "qwen3-8b".into(), provider: "qwen".into(), name: "Qwen3 8B".into(), file: "Qwen3-8B-Q4_K_M.gguf".into(), url: "https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/Qwen3-8B-Q4_K_M.gguf".into(), bytes: 5_027_783_488, layers: 36, role: "baseline".into(), note: "What the laptop can do alone".into() , mmproj: None },
        CatalogEntry { id: "gpt-oss-20b".into(), provider: "openai".into(), name: "gpt-oss 20B".into(), file: "gpt-oss-20b-mxfp4.gguf".into(), url: "https://huggingface.co/ggml-org/gpt-oss-20b-GGUF/resolve/main/gpt-oss-20b-mxfp4.gguf".into(), bytes: 12_100_000_000, layers: 24, role: "budget".into(), note: "Budget-laptop story: laptop capped to 6 GB needs one phone".into() , mmproj: None },
        CatalogEntry { id: "qwen2.5-vl-3b".into(), provider: "qwen".into(), name: "Qwen2.5-VL 3B (sees images)".into(), file: "Qwen2.5-VL-3B-Instruct-Q4_K_M.gguf".into(), url: "https://huggingface.co/ggml-org/Qwen2.5-VL-3B-Instruct-GGUF/resolve/main/Qwen2.5-VL-3B-Instruct-Q4_K_M.gguf".into(), bytes: 2_100_000_000, layers: 36, role: "vision".into(), note: "Describe photos, read screenshots, charts and documents. Needs its projector file too.".into(), mmproj: Some("mmproj-Qwen2.5-VL-3B-Instruct-f16.gguf".into()) },
        CatalogEntry { id: "qwen2.5-vl-3b-mmproj".into(), provider: "qwen".into(), name: "Qwen2.5-VL 3B projector".into(), file: "mmproj-Qwen2.5-VL-3B-Instruct-f16.gguf".into(), url: "https://huggingface.co/ggml-org/Qwen2.5-VL-3B-Instruct-GGUF/resolve/main/mmproj-Qwen2.5-VL-3B-Instruct-f16.gguf".into(), bytes: 1_400_000_000, layers: 0, role: "projector".into(), note: "The eyes of Qwen2.5-VL: download both.".into(), mmproj: None },
        CatalogEntry { id: "qwen3-coder-30b-a3b".into(), provider: "qwen".into(), name: "Qwen3 Coder 30B-A3B".into(), file: "Qwen3-Coder-30B-A3B-Instruct-Q4_K_M.gguf".into(), url: "https://huggingface.co/unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF/resolve/main/Qwen3-Coder-30B-A3B-Instruct-Q4_K_M.gguf".into(), bytes: 18_600_000_000, layers: 48, role: "headline".into(), note: "MoE: 30B knowledge, 3.3B active per token. Needs the phones.".into() , mmproj: None },
    ]
}

/// A model file name we are willing to write or serve: plain name, `.gguf`, no separators (C1).
pub fn valid_model_name(file: &str) -> bool {
    !file.is_empty()
        && file.len() < 200
        && file.ends_with(".gguf")
        && !file.starts_with('.')
        && !file.contains('/')
        && !file.contains('\\')
        && !file.contains("..")
        && file
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

pub async fn start_download(st: Arc<AppState>, url: String, file: String) -> anyhow::Result<()> {
    if !valid_model_name(&file) {
        anyhow::bail!("file name must be a plain *.gguf name");
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        anyhow::bail!("url must be http(s)");
    }
    {
        let mut d = st.downloads.write().unwrap();
        if d.iter().any(|x| x.file == file && !x.done) {
            anyhow::bail!("already downloading {file}");
        }
        d.retain(|x| x.file != file);
        d.push(Download {
            file: file.clone(),
            url: url.clone(),
            total: 0,
            bytes: 0,
            done: false,
            error: None,
            started_ms: now_ms(),
        });
    }
    let dest = st.models_dir.join(&file);
    let part = st.models_dir.join(format!("{file}.part"));
    tokio::spawn(async move {
        let res = download(&st, &url, &file, &part, &dest).await;
        let mut d = st.downloads.write().unwrap();
        if let Some(x) = d.iter_mut().find(|x| x.file == file) {
            x.done = true;
            if let Err(e) = &res {
                x.error = Some(e.to_string());
            }
        }
        drop(d);
        st.scan_models();
    });
    Ok(())
}

async fn download(
    st: &Arc<AppState>,
    url: &str,
    file: &str,
    part: &std::path::Path,
    dest: &std::path::Path,
) -> anyhow::Result<()> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    std::fs::create_dir_all(&st.models_dir)?;
    let etag_path = part.with_extension("part.etag");
    let existing = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;
    let mut req = client.get(url);
    if existing > 0 {
        req = req.header("Range", format!("bytes={existing}-"));
        // Only resume the same upstream object (M7).
        if let Ok(tag) = std::fs::read_to_string(&etag_path) {
            req = req.header("If-Range", tag.trim());
        }
    }
    let resp = req.send().await?;
    if resp.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
        let _ = std::fs::remove_file(part);
        anyhow::bail!("resume rejected by server (416); partial file removed — start again");
    }
    let resp = resp.error_for_status()?;
    let resumed = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
    if let Some(tag) = resp.headers().get("etag").and_then(|v| v.to_str().ok()) {
        let _ = std::fs::write(&etag_path, tag);
    }
    let total = resp.content_length().unwrap_or(0) + if resumed { existing } else { 0 };
    {
        let mut d = st.downloads.write().unwrap();
        if let Some(x) = d.iter_mut().find(|x| x.file == file) {
            x.total = total;
            x.bytes = if resumed { existing } else { 0 };
        }
    }
    let mut out = if resumed {
        tokio::fs::OpenOptions::new()
            .append(true)
            .open(part)
            .await?
    } else {
        tokio::fs::File::create(part).await?
    };
    let mut stream = resp.bytes_stream();
    let mut got = if resumed { existing } else { 0 };
    let mut last = std::time::Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        out.write_all(&chunk).await?;
        got += chunk.len() as u64;
        if last.elapsed().as_millis() > 300 {
            last = std::time::Instant::now();
            let mut d = st.downloads.write().unwrap();
            if let Some(x) = d.iter_mut().find(|x| x.file == file) {
                x.bytes = got;
            }
        }
    }
    out.flush().await?;
    drop(out);
    if total > 0 && got != total {
        anyhow::bail!("download ended early: {got} of {total} bytes (resume to continue)");
    }
    // The file must at least parse as GGUF before it enters the catalog.
    meshcore::gguf::read(part)
        .map_err(|e| anyhow::anyhow!("downloaded file is not a valid GGUF: {e}"))?;
    if let Some(c) = catalog().into_iter().find(|c| c.file == file) {
        let diff = (c.bytes as f64 - got as f64).abs() / c.bytes.max(1) as f64;
        if diff > 0.02 {
            tracing::warn!(
                "{file}: served size {got} differs from catalog size {} by {:.1}%",
                c.bytes,
                diff * 100.0
            );
        }
    }
    tokio::fs::rename(part, dest).await?;
    let _ = std::fs::remove_file(&etag_path);
    let mut d = st.downloads.write().unwrap();
    if let Some(x) = d.iter_mut().find(|x| x.file == file) {
        x.bytes = got;
        x.total = got;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::valid_model_name;

    #[test]
    fn rejects_traversal_and_non_gguf() {
        assert!(valid_model_name("Qwen3-0.6B-Q8_0.gguf"));
        assert!(!valid_model_name("/home/x/.bashrc"));
        assert!(!valid_model_name("../x.gguf"));
        assert!(!valid_model_name("a/b.gguf"));
        assert!(!valid_model_name("x.bin"));
        assert!(!valid_model_name(".hidden.gguf"));
        assert!(!valid_model_name("sp ace.gguf"));
    }
}
