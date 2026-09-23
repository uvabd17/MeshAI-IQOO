//! Control-plane listener: one TCP stream per device carrying length-prefixed `Envelope`s.
//! Hello(token | secret) → paired · DeviceProfile / Telemetry → state · Plan pushed on run/stop.
//!
//! Liveness: the coordinator heartbeats on a fixed 10 s interval regardless of inbound traffic
//! (N1: a `select!` sleep that is recreated every loop never fires while telemetry streams);
//! the phone times out after 45 s of silence. 45 s of silence on our side drops the link too.
//! Each accepted socket gets a connection id; a stale handler must not touch a device that has
//! since reconnected (N6). Cleanup always runs, whatever way the session ends (round-3 #5).

use crate::state::{now_ms, AppState};
use meshcore::framing;
use meshcore::proto::{envelope::Body, Envelope};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

pub const SILENCE_LIMIT_MS: u64 = 45_000;
pub const HEARTBEAT_SECS: u64 = 10;
/// Pre-auth connections are cheap to open; cap them so a LAN scanner cannot exhaust memory.
const MAX_CONNECTIONS: usize = 64;

pub async fn serve(st: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let l = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("control plane listening on :{port}");
    serve_on(st, l).await
}

pub async fn serve_on(st: Arc<AppState>, l: TcpListener) -> anyhow::Result<()> {
    let live = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    loop {
        let (sock, peer) = l.accept().await?;
        if live.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
            tracing::warn!("control: too many connections, dropping {peer}");
            continue;
        }
        let st = st.clone();
        let live = live.clone();
        live.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(async move {
            handle(st, sock, peer.ip().to_string()).await;
            live.fetch_sub(1, Ordering::SeqCst);
        });
    }
}

fn env(seq: u64, body: Body) -> Envelope {
    Envelope {
        version: 0,
        seq,
        body: Some(body),
    }
}

struct Session {
    my_conn: u64,
    link_local: Option<String>,
    device_id: Option<String>,
    seq: u64,
}

/// Runs one connection; always performs the offline cleanup afterwards.
async fn handle(st: Arc<AppState>, sock: TcpStream, ip: String) {
    let mut s = Session {
        my_conn: st.conn_seq.fetch_add(1, Ordering::SeqCst) + 1,
        link_local: sock.local_addr().map(|a| a.ip().to_string()).ok(),
        device_id: None,
        seq: 0,
    };
    if let Err(e) = session(&st, sock, &ip, &mut s).await {
        tracing::info!("control {ip} [conn {}]: {e}", s.my_conn);
    }
    cleanup(&st, &s).await;
}

async fn session(
    st: &Arc<AppState>,
    mut sock: TcpStream,
    ip: &str,
    s: &mut Session,
) -> anyhow::Result<()> {
    let _ = sock.set_nodelay(true);
    let mut buf = Vec::with_capacity(64 << 10);
    let mut tmp = vec![0u8; 64 << 10];
    let mut plan_rx = st.plan_tx.subscribe();
    let mut last_read = now_ms();
    let mut hb = tokio::time::interval(std::time::Duration::from_secs(HEARTBEAT_SECS));
    hb.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    hb.tick().await; // the first tick completes immediately
    loop {
        tokio::select! {
            n = sock.read(&mut tmp) => {
                let n = n?;
                if n == 0 { return Ok(()); }
                last_read = now_ms();
                buf.extend_from_slice(&tmp[..n]);
                while let Some(e) = framing::decode(&mut buf)? {
                    match e.body {
                        Some(Body::Hello(h)) => {
                            // First contact: redeem the one-time token and issue a secret.
                            // Reconnect: the device must prove the secret it was issued (C2).
                            let mut issued: Option<Vec<u8>> = None;
                            let ok = {
                                let mut pb = st.pairing.lock().unwrap();
                                if !h.one_time_token.is_empty() {
                                    if let Some(sec) = pb.redeem(&h.one_time_token, &h.device_id, &h.display_name) {
                                        issued = Some(sec);
                                        true
                                    } else {
                                        false
                                    }
                                } else {
                                    pb.verify(&h.device_id, &h.device_secret)
                                }
                            };
                            if !ok {
                                let reason = if h.one_time_token.is_empty() { "not paired (or wrong device secret): scan a new QR" } else { "pairing token invalid, expired, or the device id is already paired (forget it in the admin first)" };
                                sock.write_all(&framing::encode(&env(0, Body::Bye(meshcore::proto::Bye { reason: reason.into() })))).await?;
                                anyhow::bail!("rejected {} ({}): {reason}", h.device_id, ip);
                            }
                            if issued.is_some() { st.save_paired(); }
                            let resend: Option<meshcore::proto::Plan> = {
                                let mut d = st.devices.write().unwrap();
                                let rpc = if h.rpc_port == 0 { meshcore::RPC_PORT } else { h.rpc_port as u16 };
                                let e = d.entry(h.device_id.clone()).or_insert_with(|| AppState::new_remote_device(&h.device_id, &h.display_name, ip, rpc));
                                e.online = true; e.addr = Some(ip.to_string()); e.name = h.display_name.clone(); e.last_seen_ms = now_ms();
                                e.link_local_addr = s.link_local.clone(); e.conn_id = s.my_conn;
                                if h.rpc_port != 0 { e.rpc_port = h.rpc_port as u16; }
                                // A plan pushed while this device was between connections (L6).
                                let current = st.run.read().unwrap().plan_id.clone();
                                match (&e.last_plan, current) {
                                    (Some(p), Some(cur)) if p.plan_id == cur => Some(p.clone()),
                                    _ => None,
                                }
                            }; // guard dropped before the awaits below
                            s.device_id = Some(h.device_id.clone());
                            tracing::info!("{}: {} ({}) from {ip} via {} [conn {}]", if issued.is_some() { "paired" } else { "reconnected" }, h.display_name, h.device_id, s.link_local.as_deref().unwrap_or("?"), s.my_conn);
                            if let Some(secret) = issued {
                                s.seq += 1;
                                sock.write_all(&framing::encode(&env(s.seq, Body::Paired(meshcore::proto::Paired { device_secret: secret, mesh_id: st.mesh_id() })))).await?;
                            }
                            s.seq += 1;
                            sock.write_all(&framing::encode(&env(s.seq, Body::Heartbeat(meshcore::proto::Heartbeat { t_unix_ms: now_ms() })))).await?;
                            if let Some(p) = resend {
                                tracing::info!("re-sending plan {} to reconnected {}", p.plan_id, h.device_id);
                                s.seq += 1;
                                sock.write_all(&framing::encode(&env(s.seq, Body::Plan(p)))).await?;
                            }
                        }
                        Some(Body::Profile(p)) => {
                            if let Some(id) = &s.device_id {
                                if let Some(d) = st.devices.write().unwrap().get_mut(id) { d.profile = Some(p); d.last_seen_ms = now_ms(); }
                            }
                        }
                        Some(Body::Telemetry(mut t)) => {
                            if let Some(id) = &s.device_id {
                                if let Some(d) = st.devices.write().unwrap().get_mut(id) {
                                    if t.decode_tps > 0.0 { d.bench_tps = t.decode_tps; }
                                    t.device_id = id.clone();
                                    d.telemetry = Some(t); d.last_seen_ms = now_ms(); d.online = true;
                                }
                            }
                        }
                        Some(Body::Heartbeat(_)) => {
                            if let Some(id) = &s.device_id {
                                if let Some(d) = st.devices.write().unwrap().get_mut(id) { d.last_seen_ms = now_ms(); }
                            }
                        }
                        Some(Body::JobProgress(jp)) => {
                            if jp.job_id == "worker" {
                                if let Some(id) = &s.device_id {
                                    if let Some(d) = st.devices.write().unwrap().get_mut(id) {
                                        d.worker_ready_plan = Some(jp.plan_id.clone());
                                    }
                                    tracing::info!("worker ready: {id} plan {} ({})", jp.plan_id, jp.note);
                                }
                            }
                        }
                        Some(Body::JobResult(jr)) => {
                            // The phone reports a failed/exited process ("worker"/"host") or a refused plan (L2, round-3 #2).
                            if let Some(id) = &s.device_id {
                                if !jr.ok {
                                    let why = String::from_utf8_lossy(&jr.output).to_string();
                                    tracing::warn!("device {id} reports {} failure: {why}", jr.job_id);
                                    let st2 = st.clone();
                                    let id2 = id.clone();
                                    tokio::spawn(async move {
                                        crate::supervisor::fail_run_from_device(&st2, &id2, &why).await;
                                    });
                                }
                            }
                        }
                        Some(Body::JobSubmit(_)) | Some(Body::Plan(_)) | Some(Body::Paired(_)) => {}
                        Some(Body::Bye(b)) => { tracing::info!("{ip} bye: {}", b.reason); return Ok(()); }
                        None => {}
                    }
                }
            }
            r = plan_rx.recv() => {
                match r {
                    Ok((target, plan)) => {
                        if s.device_id.as_deref() == Some(target.as_str()) {
                            s.seq += 1;
                            sock.write_all(&framing::encode(&env(s.seq, Body::Plan(plan)))).await?;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("control {ip}: plan channel lagged by {n}; the current plan is re-sent on reconnect");
                    }
                    Err(_) => {}
                }
            }
            _ = hb.tick() => {
                if now_ms().saturating_sub(last_read) > SILENCE_LIMIT_MS {
                    anyhow::bail!("no traffic for {}s, dropping", SILENCE_LIMIT_MS / 1000);
                }
                s.seq += 1;
                sock.write_all(&framing::encode(&env(s.seq, Body::Heartbeat(meshcore::proto::Heartbeat { t_unix_ms: now_ms() })))).await?;
            }
        }
    }
}

/// Mark the device offline if this handler owns its current connection; if it was part of the
/// current run, end that run (H6) — under the run lock and only if the run is still the same one.
async fn cleanup(st: &Arc<AppState>, s: &Session) {
    let Some(id) = &s.device_id else { return };
    let (was_active, plan_id) = {
        let mut d = st.devices.write().unwrap();
        match d.get_mut(id) {
            Some(dev) if dev.conn_id == s.my_conn => {
                dev.online = false;
                let active = dev.role == "host" || dev.role == "worker";
                let pid = dev.last_plan.as_ref().map(|p| p.plan_id.clone());
                dev.role = "idle".into();
                dev.worker_ready_plan = None;
                dev.has_plan = false;
                (active, pid)
            }
            _ => {
                tracing::info!(
                    "stale connection {} for {id} closed; device already reconnected",
                    s.my_conn
                );
                return;
            }
        }
    };
    tracing::info!("device offline: {id} [conn {}]", s.my_conn);
    if was_active {
        let _g = st.run_lock.lock().await;
        let current = st.run.read().unwrap().plan_id.clone();
        if current.is_some() && current == plan_id {
            st.push_log(format!(
                "device {id} went offline during the run — stopping"
            ));
            crate::supervisor::stop(st).await;
            let mut r = st.run.write().unwrap();
            r.status = "error".into();
            r.error = Some(format!("device {id} disconnected"));
        }
    }
}

#[cfg(test)]
mod tests {
    //! Laptop-only proof of the round-2 CRITICAL fix: a fake phone pairs, streams telemetry every
    //! 2 s (which starves a naive `select!` sleep) and must still receive heartbeats on the 10 s
    //! interval; a second connection for the same device must supersede the first without the
    //! first's exit marking the device offline.
    use super::*;
    use meshcore::proto::{Hello, Telemetry};
    use tokio::io::AsyncReadExt;

    async fn read_env(sock: &mut TcpStream, buf: &mut Vec<u8>) -> Envelope {
        let mut tmp = [0u8; 4096];
        loop {
            if let Some(e) = framing::decode(buf).unwrap() {
                return e;
            }
            let n = sock.read(&mut tmp).await.unwrap();
            assert!(n > 0, "socket closed");
            buf.extend_from_slice(&tmp[..n]);
        }
    }

    fn test_state() -> Arc<AppState> {
        let tmp = std::env::temp_dir().join(format!("meshai-ctl-{}", now_ms()));
        Arc::new(AppState::new(tmp.clone(), tmp.clone(), tmp, false, None))
    }

    #[tokio::test]
    async fn heartbeats_keep_flowing_while_telemetry_streams() {
        let st = test_state();
        let l = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        let st2 = st.clone();
        tokio::spawn(async move { serve_on(st2, l).await.unwrap() });
        let offer = st.new_offer(port);

        let mut sock = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let hello = env(
            1,
            Body::Hello(Hello {
                device_id: "test-phone".into(),
                display_name: "Fake".into(),
                one_time_token: offer.token.as_bytes().to_vec(),
                rpc_port: 50052,
                device_secret: vec![],
            }),
        );
        sock.write_all(&framing::encode(&hello)).await.unwrap();
        let mut buf = Vec::new();
        let first = read_env(&mut sock, &mut buf).await;
        let secret = match first.body {
            Some(Body::Paired(p)) => p.device_secret,
            other => panic!("expected Paired, got {other:?}"),
        };
        assert_eq!(secret.len(), 32);

        // Stream telemetry every 2 s for 25 s and count heartbeats: with a 10 s interval we must see >= 2.
        let start = std::time::Instant::now();
        let mut heartbeats = 0;
        let mut seq = 1;
        while start.elapsed() < std::time::Duration::from_secs(25) {
            seq += 1;
            let t = env(
                seq,
                Body::Telemetry(Telemetry {
                    device_id: "test-phone".into(),
                    avail_bytes: 4_000_000_000,
                    ..Default::default()
                }),
            );
            sock.write_all(&framing::encode(&t)).await.unwrap();
            let deadline = tokio::time::sleep(std::time::Duration::from_secs(2));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    _ = &mut deadline => break,
                    e = read_env(&mut sock, &mut buf) => { if matches!(e.body, Some(Body::Heartbeat(_))) { heartbeats += 1; } }
                }
            }
        }
        assert!(
            heartbeats >= 2,
            "expected >= 2 heartbeats in 25 s, got {heartbeats}"
        );
        assert!(st
            .devices
            .read()
            .unwrap()
            .get("test-phone")
            .map(|d| d.online)
            .unwrap_or(false));

        // Reconnect with the secret on a second socket; closing the FIRST socket must not mark the device offline.
        let mut sock2 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let hello2 = env(
            1,
            Body::Hello(Hello {
                device_id: "test-phone".into(),
                display_name: "Fake".into(),
                one_time_token: vec![],
                rpc_port: 50052,
                device_secret: secret.clone(),
            }),
        );
        sock2.write_all(&framing::encode(&hello2)).await.unwrap();
        let mut buf2 = Vec::new();
        let ack = read_env(&mut sock2, &mut buf2).await;
        assert!(
            matches!(ack.body, Some(Body::Heartbeat(_))),
            "reconnect with secret must be accepted"
        );
        drop(sock);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        assert!(
            st.devices.read().unwrap().get("test-phone").unwrap().online,
            "stale socket close must not mark the reconnected device offline"
        );

        // A wrong secret is refused with Bye.
        let mut sock3 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let bad = env(
            1,
            Body::Hello(Hello {
                device_id: "test-phone".into(),
                display_name: "Fake".into(),
                one_time_token: vec![],
                rpc_port: 50052,
                device_secret: vec![0u8; 32],
            }),
        );
        sock3.write_all(&framing::encode(&bad)).await.unwrap();
        let mut buf3 = Vec::new();
        let r = read_env(&mut sock3, &mut buf3).await;
        assert!(matches!(r.body, Some(Body::Bye(_))));
    }
}
