//! Model catalog: known demo models, HTTP downloads with progress, file serving for phones.

use crate::state::{now_ms, AppState, Download};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub provider: String, // qwen | openai | anthropic | huggingface
    pub name: String,
    pub file: String,
    pub url: String,
    pub bytes: u64,
    pub layers: u32,
    pub role: String, // baseline | budget | headline | test | cloud
    pub note: String,
}

/// The demo set (D012). Sizes are from the model cards; verified on download.
pub fn catalog() -> Vec<CatalogEntry> {
    vec![
        CatalogEntry { id: "qwen3-0.6b".into(), provider: "qwen".into(), name: "Qwen3 0.6B".into(), file: "Qwen3-0.6B-Q8_0.gguf".into(), url: "https://huggingface.co/Qwen/Qwen3-0.6B-GGUF/resolve/main/Qwen3-0.6B-Q8_0.gguf".into(), bytes: 639_446_688, layers: 28, role: "test".into(), note: "Split tests and CI".into() },
        CatalogEntry { id: "qwen3-8b".into(), provider: "qwen".into(), name: "Qwen3 8B".into(), file: "Qwen3-8B-Q4_K_M.gguf".into(), url: "https://huggingface.co/Qwen/Qwen3-8B-GGUF/resolve/main/Qwen3-8B-Q4_K_M.gguf".into(), bytes: 5_030_000_000, layers: 36, role: "baseline".into(), note: "What the laptop can do alone".into() },
        CatalogEntry { id: "gpt-oss-20b".into(), provider: "openai".into(), name: "gpt-oss 20B".into(), file: "gpt-oss-20b-mxfp4.gguf".into(), url: "https://huggingface.co/ggml-org/gpt-oss-20b-GGUF/resolve/main/gpt-oss-20b-mxfp4.gguf".into(), bytes: 12_100_000_000, layers: 24, role: "budget".into(), note: "Budget-laptop story: laptop capped to 6 GB needs one phone".into() },
        CatalogEntry { id: "qwen3-coder-30b-a3b".into(), provider: "qwen".into(), name: "Qwen3 Coder 30B-A3B".into(), file: "Qwen3-Coder-30B-A3B-Instruct-Q4_K_M.gguf".into(), url: "https://huggingface.co/unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF/resolve/main/Qwen3-Coder-30B-A3B-Instruct-Q4_K_M.gguf".into(), bytes: 18_600_000_000, layers: 48, role: "headline".into(), note: "MoE: 30B knowledge, 3.3B active per token. Needs the phones.".into() },
    ]
}

pub async fn start_download(st: Arc<AppState>, url: String, file: String) -> anyhow::Result<()> {
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
    let existing = std::fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?;
    let mut req = client.get(url);
    if existing > 0 {
        req = req.header("Range", format!("bytes={existing}-"));
    }
    let resp = req.send().await?.error_for_status()?;
    let resumed = resp.status() == reqwest::StatusCode::PARTIAL_CONTENT;
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
    tokio::fs::rename(part, dest).await?;
    let mut d = st.downloads.write().unwrap();
    if let Some(x) = d.iter_mut().find(|x| x.file == file) {
        x.bytes = got;
        x.total = got;
    }
    Ok(())
}
