//! Control-plane listener: one TCP stream per device carrying length-prefixed `Envelope`s.
//! Hello(token | secret) → paired · DeviceProfile / Telemetry → state · Plan pushed on run/stop.
//!
//! Liveness: the coordinator heartbeats on a fixed 10 s interval regardless of inbound traffic
//! (N1: a `select!` sleep that is recreated every loop never fires while telemetry streams);
//! the phone times out after 45 s of silence. 45 s of silence on our side drops the link too.
//! Each accepted socket gets a connection id; a stale handler must not touch a device that has
//! since reconnected (N6).

use crate::state::{now_ms, AppState};
use meshcore::framing;
use meshcore::proto::{envelope::Body, Envelope};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const SILENCE_LIMIT_MS: u64 = 45_000;
const HEARTBEAT_SECS: u64 = 10;

pub async fn serve(st: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let l = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("control plane listening on :{port}");
    loop {
        let (sock, peer) = l.accept().await?;
        let st = st.clone();
        tokio::spawn(async move {
            if let Err(e) = handle(st, sock, peer.ip().to_string()).await {
                tracing::info!("control {peer}: {e}");
            }
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

async fn handle(st: Arc<AppState>, mut sock: TcpStream, ip: String) -> anyhow::Result<()> {
    let _ = sock.set_nodelay(true);
    let my_conn = st.conn_seq.fetch_add(1, Ordering::SeqCst) + 1;
    let link_local = sock.local_addr().map(|a| a.ip().to_string()).ok();
    let mut buf = Vec::with_capacity(64 << 10);
    let mut tmp = vec![0u8; 64 << 10];
    let mut device_id: Option<String> = None;
    let mut plan_rx = st.plan_tx.subscribe();
    let mut seq = 0u64;
    let mut last_read = now_ms();
    let mut hb = tokio::time::interval(std::time::Duration::from_secs(HEARTBEAT_SECS));
    hb.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    hb.tick().await; // the first tick completes immediately
    loop {
        tokio::select! {
            n = sock.read(&mut tmp) => {
                let n = n?;
                if n == 0 { break; }
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
                                    if let Some(s) = pb.redeem(&h.one_time_token, &h.device_id, &h.display_name) {
                                        issued = Some(s);
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
                            {
                                let mut d = st.devices.write().unwrap();
                                let rpc = if h.rpc_port == 0 { meshcore::RPC_PORT } else { h.rpc_port as u16 };
                                let e = d.entry(h.device_id.clone()).or_insert_with(|| AppState::new_remote_device(&h.device_id, &h.display_name, &ip, rpc));
                                e.online = true; e.addr = Some(ip.clone()); e.name = h.display_name.clone(); e.last_seen_ms = now_ms();
                                e.link_local_addr = link_local.clone(); e.conn_id = my_conn;
                                if h.rpc_port != 0 { e.rpc_port = h.rpc_port as u16; }
                            } // guard dropped before the await below
                            device_id = Some(h.device_id.clone());
                            tracing::info!("{}: {} ({}) from {ip} via {} [conn {my_conn}]", if issued.is_some() { "paired" } else { "reconnected" }, h.display_name, h.device_id, link_local.as_deref().unwrap_or("?"));
                            if let Some(secret) = issued {
                                seq += 1;
                                sock.write_all(&framing::encode(&env(seq, Body::Paired(meshcore::proto::Paired { device_secret: secret, mesh_id: st.mesh_id() })))).await?;
                            }
                            seq += 1;
                            sock.write_all(&framing::encode(&env(seq, Body::Heartbeat(meshcore::proto::Heartbeat { t_unix_ms: now_ms() })))).await?;
                        }
                        Some(Body::Profile(p)) => {
                            if let Some(id) = &device_id {
                                if let Some(d) = st.devices.write().unwrap().get_mut(id) { d.profile = Some(p); d.last_seen_ms = now_ms(); }
                            }
                        }
                        Some(Body::Telemetry(mut t)) => {
                            if let Some(id) = &device_id {
                                if let Some(d) = st.devices.write().unwrap().get_mut(id) {
                                    if t.decode_tps > 0.0 { d.bench_tps = t.decode_tps; }
                                    t.device_id = id.clone();
                                    d.telemetry = Some(t); d.last_seen_ms = now_ms(); d.online = true;
                                }
                            }
                        }
                        Some(Body::Heartbeat(_)) => {
                            if let Some(id) = &device_id {
                                if let Some(d) = st.devices.write().unwrap().get_mut(id) { d.last_seen_ms = now_ms(); }
                            }
                        }
                        Some(Body::JobProgress(jp)) => {
                            if jp.job_id == "worker" {
                                if let Some(id) = &device_id {
                                    if let Some(d) = st.devices.write().unwrap().get_mut(id) {
                                        d.worker_ready_plan = Some(jp.plan_id.clone());
                                    }
                                    tracing::info!("worker ready: {id} plan {} ({})", jp.plan_id, jp.note);
                                }
                            }
                        }
                        Some(Body::JobResult(_)) | Some(Body::JobSubmit(_)) | Some(Body::Plan(_)) | Some(Body::Paired(_)) => {}
                        Some(Body::Bye(b)) => { tracing::info!("{ip} bye: {}", b.reason); break; }
                        None => {}
                    }
                }
            }
            r = plan_rx.recv() => {
                if let Ok((target, plan)) = r {
                    if device_id.as_deref() == Some(target.as_str()) {
                        seq += 1;
                        sock.write_all(&framing::encode(&env(seq, Body::Plan(plan)))).await?;
                    }
                }
            }
            _ = hb.tick() => {
                if now_ms().saturating_sub(last_read) > SILENCE_LIMIT_MS {
                    tracing::info!("{ip}: no traffic for {}s, dropping", SILENCE_LIMIT_MS / 1000);
                    break;
                }
                seq += 1;
                sock.write_all(&framing::encode(&env(seq, Body::Heartbeat(meshcore::proto::Heartbeat { t_unix_ms: now_ms() })))).await?;
            }
        }
    }
    if let Some(id) = device_id {
        // Only the handler that owns the device's current connection may mark it offline.
        let was_active = {
            let mut d = st.devices.write().unwrap();
            match d.get_mut(&id) {
                Some(dev) if dev.conn_id == my_conn => {
                    dev.online = false;
                    let active = dev.role == "host" || dev.role == "worker";
                    dev.role = "idle".into();
                    dev.worker_ready_plan = None;
                    dev.has_plan = false;
                    active
                }
                _ => {
                    tracing::info!(
                        "stale connection {my_conn} for {id} closed; device already reconnected"
                    );
                    return Ok(());
                }
            }
        };
        tracing::info!("device offline: {id} [conn {my_conn}]");
        // A participant vanished mid-run: llama.cpp will abort on the next RPC call, so end the run now (H6).
        if was_active {
            let st2 = st.clone();
            tokio::spawn(async move {
                st2.push_log(format!(
                    "device {id} went offline during the run — stopping"
                ));
                crate::supervisor::stop(&st2).await;
                let mut r = st2.run.write().unwrap();
                r.status = "error".into();
                r.error = Some(format!("device {id} disconnected"));
            });
        }
    }
    Ok(())
}
