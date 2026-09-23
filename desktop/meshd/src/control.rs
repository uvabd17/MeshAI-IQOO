//! Control-plane listener: one TCP stream per device carrying length-prefixed `Envelope`s.
//! Hello(token) → paired · DeviceProfile / Telemetry → state · Plan pushed on run/stop.

use crate::state::{now_ms, AppState, Device, DeviceKind};
use meshcore::framing;
use meshcore::proto::{envelope::Body, Envelope};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

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

async fn handle(st: Arc<AppState>, mut sock: TcpStream, ip: String) -> anyhow::Result<()> {
    let mut buf = Vec::with_capacity(64 << 10);
    let mut tmp = vec![0u8; 64 << 10];
    let mut device_id: Option<String> = None;
    let mut plan_rx = st.plan_tx.subscribe();
    let mut seq = 0u64;
    loop {
        tokio::select! {
            n = sock.read(&mut tmp) => {
                let n = n?;
                if n == 0 { break; }
                buf.extend_from_slice(&tmp[..n]);
                while let Some(env) = framing::decode(&mut buf)? {
                    match env.body {
                        Some(Body::Hello(h)) => {
                            let ok = st.pairing.lock().unwrap().redeem(&h.one_time_token, &h.device_id, &h.display_name)
                                || st.pairing.lock().unwrap().is_paired(&h.device_id);
                            if !ok {
                                let bye = Envelope { version: 0, seq: 0, body: Some(Body::Bye(meshcore::proto::Bye { reason: "pairing token invalid or expired".into() })) };
                                sock.write_all(&framing::encode(&bye)).await?;
                                anyhow::bail!("rejected {} ({})", h.device_id, ip);
                            }
                            {
                                let mut d = st.devices.write().unwrap();
                                let e = d.entry(h.device_id.clone()).or_insert_with(|| Device {
                                    id: h.device_id.clone(), name: h.display_name.clone(), kind: DeviceKind::Phone, is_local: false, online: true,
                                    addr: Some(ip.clone()), rpc_port: if h.rpc_port == 0 { meshcore::RPC_PORT } else { h.rpc_port as u16 }, role: "idle".into(),
                                    profile: None, telemetry: None, usable_override_bytes: None, last_seen_ms: now_ms(), bench_tps: 0.0,
                                });
                                e.online = true; e.addr = Some(ip.clone()); e.name = h.display_name.clone(); e.last_seen_ms = now_ms();
                                if h.rpc_port != 0 { e.rpc_port = h.rpc_port as u16; }
                            } // guard dropped before the await below
                            device_id = Some(h.device_id.clone());
                            tracing::info!("paired: {} ({}) from {ip}", h.display_name, h.device_id);
                            // ack with a heartbeat
                            let hb = Envelope { version: 0, seq: 0, body: Some(Body::Heartbeat(meshcore::proto::Heartbeat { t_unix_ms: now_ms() })) };
                            sock.write_all(&framing::encode(&hb)).await?;
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
                        Some(Body::JobProgress(_)) | Some(Body::JobResult(_)) | Some(Body::JobSubmit(_)) | Some(Body::Plan(_)) => {}
                        Some(Body::Bye(b)) => { tracing::info!("{ip} bye: {}", b.reason); break; }
                        None => {}
                    }
                }
            }
            r = plan_rx.recv() => {
                if let Ok((target, plan)) = r {
                    if device_id.as_deref() == Some(target.as_str()) {
                        seq += 1;
                        let env = Envelope { version: 0, seq, body: Some(Body::Plan(plan)) };
                        sock.write_all(&framing::encode(&env)).await?;
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                seq += 1;
                let hb = Envelope { version: 0, seq, body: Some(Body::Heartbeat(meshcore::proto::Heartbeat { t_unix_ms: now_ms() })) };
                sock.write_all(&framing::encode(&hb)).await?;
            }
        }
    }
    if let Some(id) = device_id {
        if let Some(d) = st.devices.write().unwrap().get_mut(&id) {
            d.online = false;
            d.role = "idle".into();
        }
        tracing::info!("device offline: {id}");
    }
    Ok(())
}
