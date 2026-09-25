//! `/v1/*` reverse proxy to the current host's llama-server, measuring TTFT / tokens / tok/s
//! per request client-side (what the user actually experiences) and recording a `RunRow`.

use crate::state::{now_ms, AppState, RunRow};
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use std::sync::Arc;

pub async fn v1(State(st): State<Arc<AppState>>, req: Request) -> Response {
    // One guard (M1): taking a second read lock in a match guard can deadlock against a writer.
    let endpoint = {
        let run = st.run.read().unwrap();
        if run.status == "ready" {
            run.endpoint.clone()
        } else {
            None
        }
    };
    let Some(endpoint) = endpoint else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "no model is running — start one in the admin panel",
        )
            .into_response();
    };
    let path_q = req
        .uri()
        .path_and_query()
        .map(|p| p.to_string())
        .unwrap_or_else(|| "/".into());
    let url = format!("{endpoint}{path_q}");
    let method = req.method().clone();
    let headers = req.headers().clone();
    let body_bytes = match axum::body::to_bytes(req.into_body(), 64 << 20).await {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    let is_chat = path_q.contains("/chat/completions") || path_q.contains("/completions");
    let streaming = is_chat
        && serde_json::from_slice::<serde_json::Value>(&body_bytes)
            .map(|v| v["stream"].as_bool().unwrap_or(false))
            .unwrap_or(false);

    let client = reqwest::Client::new();
    let mut rb = client.request(method, &url);
    for (k, v) in headers.iter() {
        if k != "host" && k != "content-length" && k != "authorization" && k != "x-mesh-token" {
            rb = rb.header(k, v);
        }
    }
    let t0 = std::time::Instant::now();
    let resp = match rb.body(body_bytes.clone()).send().await {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, format!("host unreachable: {e}")).into_response()
        }
    };
    let status = resp.status();
    let mut out_headers = HeaderMap::new();
    for (k, v) in resp.headers().iter() {
        if k != "transfer-encoding" && k != "content-length" {
            out_headers.insert(k.clone(), v.clone());
        }
    }
    let (model, mode, host, ndev) = {
        let p = st.plan.read().unwrap();
        match p.as_ref() {
            Some(p) => (
                p.model.clone(),
                format!("{:?}", p.mode),
                p.host_id.clone(),
                p.placements
                    .iter()
                    .filter(|x| x.role != meshcore::planner::Role::Rejected)
                    .count(),
            ),
            None => ("?".into(), "?".into(), "?".into(), 0),
        }
    };

    if !is_chat {
        let bytes = resp.bytes().await.unwrap_or_default();
        return (status, out_headers, bytes).into_response();
    }

    if streaming {
        let st2 = st.clone();
        let mut first: Option<u64> = None;
        let mut tokens = 0u32;
        let mut prompt_tokens = 0u32;
        let mut usage_prompt_tps = 0f32;
        let mut carry = String::new(); // SSE events can straddle TCP chunks (M6)
        let mut done = false;
        let stream = resp.bytes_stream().map(move |chunk| {
            if let Ok(c) = &chunk {
                if first.is_none() && c.iter().any(|b| !b.is_ascii_whitespace()) {
                    first = Some(t0.elapsed().as_millis() as u64);
                }
                carry.push_str(&String::from_utf8_lossy(c));
                while let Some(nl) = carry.find('\n') {
                    let line = carry[..nl].trim_end_matches('\r').to_string();
                    carry.drain(..=nl);
                    let Some(json) = line.strip_prefix("data: ") else {
                        continue;
                    };
                    if json.trim() == "[DONE]" {
                        if done {
                            continue;
                        }
                        done = true;
                        let total = t0.elapsed().as_millis() as u64;
                        let ttft = first.unwrap_or(total);
                        let gen_ms = total.saturating_sub(ttft).max(1);
                        // n tokens span n-1 inter-token intervals after the first.
                        let tps = if tokens > 1 {
                            (tokens - 1) as f32 * 1000.0 / gen_ms as f32
                        } else {
                            0.0
                        };
                        st2.record_run(RunRow {
                            ts_ms: now_ms(),
                            model: model.clone(),
                            mode: mode.clone(),
                            host: host.clone(),
                            devices: ndev,
                            ttft_ms: ttft,
                            total_ms: total,
                            tokens_out: tokens,
                            prompt_tokens,
                            tps,
                            prompt_tps: usage_prompt_tps,
                            ok: true,
                            streaming: true,
                            link: String::new(),
                        });
                    } else if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
                        let d = &v["choices"][0]["delta"];
                        if d["content"].is_string()
                            || d["reasoning_content"].is_string()
                            || v["choices"][0]["text"].is_string()
                        {
                            tokens += 1;
                        }
                        if let Some(u) = v.get("usage") {
                            prompt_tokens = u["prompt_tokens"].as_u64().unwrap_or(0) as u32;
                            if let Some(ct) = u["completion_tokens"].as_u64() {
                                if ct > 0 {
                                    tokens = ct as u32;
                                }
                            }
                        }
                        if let Some(t) = v.get("timings") {
                            if let Some(p) = t["prompt_per_second"].as_f64() {
                                usage_prompt_tps = p as f32;
                            }
                        }
                    }
                }
            }
            chunk
        });
        return (status, out_headers, Body::from_stream(stream)).into_response();
    }

    let bytes = resp.bytes().await.unwrap_or_default();
    let total = t0.elapsed().as_millis() as u64;
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    let tokens = v["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;
    let prompt_tokens = v["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
    let timings = v.get("timings").cloned().unwrap_or_default();
    let tps = timings["predicted_per_second"]
        .as_f64()
        .map(|x| x as f32)
        .unwrap_or(tokens as f32 * 1000.0 / total.max(1) as f32);
    let prompt_tps = timings["prompt_per_second"].as_f64().unwrap_or(0.0) as f32;
    let ttft = timings["prompt_ms"].as_f64().map(|x| x as u64).unwrap_or(0);
    st.record_run(RunRow {
        ts_ms: now_ms(),
        model,
        mode,
        host,
        devices: ndev,
        ttft_ms: ttft,
        total_ms: total,
        tokens_out: tokens,
        prompt_tokens,
        tps,
        prompt_tps,
        ok: status.is_success(),
        streaming: false,
        link: String::new(),
    });
    (status, out_headers, bytes).into_response()
}
