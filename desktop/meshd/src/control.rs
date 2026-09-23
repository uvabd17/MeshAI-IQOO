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
/// Unauthenticated sockets have their own, smaller pool so they can never crowd out a paired
/// phone (round-5 #5).
const MAX_CONNECTIONS: usize = 64;
const MAX_PREAUTH: usize = 16;
/// Per source address, so one LAN peer cycling silent sockets cannot fill the pre-auth pool and
/// lock a reconnecting phone out (round-6 #5).
const MAX_PREAUTH_PER_IP: usize = 4;

#[derive(Default)]
struct PreauthPool {
    total: std::sync::atomic::AtomicUsize,
    per_ip: std::sync::Mutex<std::collections::HashMap<std::net::IpAddr, usize>>,
}

impl PreauthPool {
    /// Take a slot for `ip`; `false` if the pool or that address is full.
    fn acquire(&self, ip: std::net::IpAddr) -> bool {
        let mut m = self.per_ip.lock().unwrap();
        if self.total.load(Ordering::SeqCst) >= MAX_PREAUTH
            || m.get(&ip).copied().unwrap_or(0) >= MAX_PREAUTH_PER_IP
        {
            return false; // no entry is inserted for a refused address (round-7 #4)
        }
        let n = m.entry(ip).or_insert(0);
        *n += 1;
        self.total.fetch_add(1, Ordering::SeqCst);
        true
    }
    fn release(&self, ip: std::net::IpAddr) {
        let mut m = self.per_ip.lock().unwrap();
        if let Some(n) = m.get_mut(&ip) {
            *n -= 1;
            if *n == 0 {
                m.remove(&ip);
            }
        }
        self.total.fetch_sub(1, Ordering::SeqCst);
    }
}
/// Before Hello a frame may be at most this big and Hello must arrive within this deadline, so 64
/// unauthenticated sockets can hold at most 64 × 4 KiB, not 64 × 16 MiB (round-4 #9).
const PREAUTH_MAX_FRAME: usize = 4096;
const HELLO_DEADLINE_SECS: u64 = 5;

pub async fn serve(st: Arc<AppState>, port: u16) -> anyhow::Result<()> {
    let l = TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("control plane listening on :{port}");
    serve_on(st, l).await
}

pub async fn serve_on(st: Arc<AppState>, l: TcpListener) -> anyhow::Result<()> {
    let live = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let preauth = Arc::new(PreauthPool::default());
    loop {
        let (sock, peer) = l.accept().await?;
        if live.load(Ordering::SeqCst) >= MAX_CONNECTIONS || !preauth.acquire(peer.ip()) {
            tracing::warn!("control: too many (pre-auth) connections, dropping {peer}");
            continue;
        }
        let st = st.clone();
        let live = live.clone();
        let preauth = preauth.clone();
        live.fetch_add(1, Ordering::SeqCst);
        tokio::spawn(async move {
            handle(st, sock, peer.ip(), preauth).await;
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
    /// Pre-auth pool slot; released once Hello succeeds or when the socket closes unpaired.
    preauth: Option<(Arc<PreauthPool>, std::net::IpAddr)>,
}

impl Session {
    fn authenticated(&mut self) {
        if let Some((p, ip)) = self.preauth.take() {
            p.release(ip);
        }
    }
}

/// Runs one connection; always performs the offline cleanup afterwards.
async fn handle(
    st: Arc<AppState>,
    sock: TcpStream,
    ip: std::net::IpAddr,
    preauth: Arc<PreauthPool>,
) {
    let mut s = Session {
        my_conn: st.conn_seq.fetch_add(1, Ordering::SeqCst) + 1,
        link_local: sock.local_addr().map(|a| a.ip().to_string()).ok(),
        device_id: None,
        seq: 0,
        preauth: Some((preauth, ip)),
    };
    let ip = ip.to_string();
    if let Err(e) = session(&st, sock, &ip, &mut s).await {
        tracing::info!("control {ip} [conn {}]: {e}", s.my_conn);
    }
    s.authenticated(); // release the pre-auth slot if Hello never succeeded
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
    let mut ctl_rx = st.session_ctl.subscribe();
    let hello_deadline = tokio::time::sleep(std::time::Duration::from_secs(HELLO_DEADLINE_SECS));
    tokio::pin!(hello_deadline);
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
                if s.device_id.is_none() && buf.len() >= 4 {
                    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
                    if len > PREAUTH_MAX_FRAME { anyhow::bail!("pre-auth frame of {len} bytes refused"); }
                }
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
                                // Re-check under the devices guard: a Forget that ran between verify and
                                // here must not resurrect the device as a ghost entry (round-6 #6).
                                if !st.pairing.lock().unwrap().is_paired(&h.device_id) {
                                    drop(d);
                                    sock.write_all(&framing::encode(&env(0, Body::Bye(meshcore::proto::Bye { reason: "forgotten by the coordinator".into() })))).await?;
                                    anyhow::bail!("{} was forgotten during its Hello", h.device_id);
                                }
                                let rpc = if h.rpc_port == 0 { meshcore::RPC_PORT } else { h.rpc_port as u16 };
                                let e = d.entry(h.device_id.clone()).or_insert_with(|| AppState::new_remote_device(&h.device_id, &h.display_name, ip, rpc));
                                e.online = true; e.addr = Some(ip.to_string()); e.name = h.display_name.clone(); e.last_seen_ms = now_ms();
                                e.link_local_addr = s.link_local.clone(); e.conn_id = s.my_conn;
                                if h.rpc_port != 0 { e.rpc_port = h.rpc_port as u16; }
                                // A plan pushed while this device was between connections (L6).
                                let current = st.run.read().unwrap().plan_id.clone();
                                match (&e.last_plan, current) {
                                    (Some(p), Some(cur)) if p.plan_id == cur => {
                                        // Restore membership fully: has_plan *and* role (round-5 #2).
                                        e.has_plan = true;
                                        let mine = p.placements.iter().find(|pl| pl.device_id == h.device_id && pl.used);
                                        e.role = match mine { Some(pl) if pl.is_host => "host", Some(_) => "worker", None => "idle" }.into();
                                        Some(p.clone())
                                    }
                                    _ => None,
                                }
                            }; // guard dropped before the awaits below
                            s.device_id = Some(h.device_id.clone());
                            s.authenticated();
                            let _ = st.session_ctl.send((h.device_id.clone(), s.my_conn));
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
                            if let Some(id) = owned_id(st, s) {
                                if let Some(d) = st.devices.write().unwrap().get_mut(&id) { d.profile = Some(p); d.last_seen_ms = now_ms(); }
                            }
                        }
                        Some(Body::Telemetry(mut t)) => {
                            if let Some(id) = owned_id(st, s) {
                                if let Some(d) = st.devices.write().unwrap().get_mut(&id) {
                                    if t.decode_tps > 0.0 { d.bench_tps = t.decode_tps; }
                                    t.device_id = id.clone();
                                    d.telemetry = Some(t); d.last_seen_ms = now_ms(); d.online = true;
                                }
                            }
                        }
                        Some(Body::Heartbeat(_)) => {
                            if let Some(id) = owned_id(st, s) {
                                if let Some(d) = st.devices.write().unwrap().get_mut(&id) { d.last_seen_ms = now_ms(); }
                            }
                        }
                        Some(Body::JobProgress(jp)) => {
                            if jp.job_id == "worker" {
                                if let Some(id) = owned_id(st, s) {
                                    if let Some(d) = st.devices.write().unwrap().get_mut(&id) {
                                        d.worker_ready_plan = Some(jp.plan_id.clone());
                                    }
                                    tracing::info!("worker ready: {id} plan {} ({})", jp.plan_id, jp.note);
                                }
                            }
                        }
                        Some(Body::JobResult(jr)) => {
                            // The phone reports a failed/exited process ("worker"/"host") or a refused plan (L2, round-3 #2).
                            // Only honoured for the current plan id (round-4 #1) and from the owning connection.
                            if let Some(id) = owned_id(st, s) {
                                if !jr.ok {
                                    let why = String::from_utf8_lossy(&jr.output).to_string();
                                    tracing::warn!("device {id} reports {} failure (plan {}): {why}", jr.job_id, jr.plan_id);
                                    let st2 = st.clone();
                                    let plan_id = jr.plan_id.clone();
                                    tokio::spawn(async move {
                                        crate::supervisor::fail_run_from_device(&st2, &id, &plan_id, &why).await;
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
                        // Only the connection that currently owns the device forwards plans (round-4 #8).
                        if s.device_id.as_deref() == Some(target.as_str()) && owned_id(st, s).is_some() {
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
            r = ctl_rx.recv() => {
                if let Ok((target, conn)) = r {
                    if s.device_id.as_deref() == Some(target.as_str()) && conn != s.my_conn {
                        if conn == 0 {
                            sock.write_all(&framing::encode(&env(0, Body::Bye(meshcore::proto::Bye { reason: "forgotten by the coordinator".into() })))).await?;
                            anyhow::bail!("device forgotten; closing");
                        }
                        anyhow::bail!("superseded by conn {conn}; closing");
                    }
                }
            }
            _ = &mut hello_deadline, if s.device_id.is_none() => {
                anyhow::bail!("no Hello within {HELLO_DEADLINE_SECS}s, dropping");
            }
            _ = hb.tick() => {
                let limit = st.silence_limit_ms.load(Ordering::Relaxed);
                if now_ms().saturating_sub(last_read) > limit {
                    anyhow::bail!("no traffic for {}s, dropping", limit / 1000);
                }
                s.seq += 1;
                sock.write_all(&framing::encode(&env(s.seq, Body::Heartbeat(meshcore::proto::Heartbeat { t_unix_ms: now_ms() })))).await?;
            }
        }
    }
}

/// The device id if this connection still owns the device (a reconnect takes ownership, N6).
fn owned_id(st: &AppState, s: &Session) -> Option<String> {
    let id = s.device_id.as_ref()?;
    let d = st.devices.read().unwrap();
    (d.get(id)?.conn_id == s.my_conn).then(|| id.clone())
}

/// Mark the device offline if this handler owns its current connection; if it was part of the
/// current run, end that run (H6) — under the run lock, only if the run is still the same one and
/// only if the device has not reconnected in the meantime (round-4 #4).
async fn cleanup(st: &Arc<AppState>, s: &Session) {
    let Some(id) = &s.device_id else { return };
    let (was_active, plan_id) = {
        let mut d = st.devices.write().unwrap();
        match d.get_mut(id) {
            Some(dev) if dev.conn_id == s.my_conn => {
                dev.online = false;
                let active = dev.in_run();
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
        if owned_id(st, s).is_none() {
            tracing::info!("{id} reconnected while its cleanup waited; run left alone");
            return;
        }
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

    use meshcore::proto::{JobResult, Plan};

    async fn read_env(sock: &mut TcpStream, buf: &mut Vec<u8>) -> Envelope {
        try_read_env(sock, buf).await.expect("socket closed")
    }

    /// `None` when the peer closed the socket.
    async fn try_read_env(sock: &mut TcpStream, buf: &mut Vec<u8>) -> Option<Envelope> {
        let mut tmp = [0u8; 4096];
        loop {
            if let Some(e) = framing::decode(buf).unwrap() {
                return Some(e);
            }
            let n = sock.read(&mut tmp).await.ok()?;
            if n == 0 {
                return None;
            }
            buf.extend_from_slice(&tmp[..n]);
        }
    }

    fn test_state() -> Arc<AppState> {
        let tmp =
            std::env::temp_dir().join(format!("meshai-ctl-{}-{}", now_ms(), rand::random::<u32>()));
        Arc::new(AppState::new(tmp.clone(), tmp.clone(), tmp, false, None))
    }

    /// Start a listener, pair a fake phone with a fresh offer; returns (port, socket, secret, buf).
    async fn pair(st: &Arc<AppState>, id: &str) -> (u16, TcpStream, Vec<u8>, Vec<u8>) {
        let l = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        let st2 = st.clone();
        tokio::spawn(async move { serve_on(st2, l).await.unwrap() });
        let offer = st.new_offer(port);
        let mut sock = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let hello = env(
            1,
            Body::Hello(Hello {
                device_id: id.into(),
                display_name: "Fake".into(),
                one_time_token: offer.token.as_bytes().to_vec(),
                rpc_port: 50052,
                device_secret: vec![],
            }),
        );
        sock.write_all(&framing::encode(&hello)).await.unwrap();
        let mut buf = Vec::new();
        let secret = match read_env(&mut sock, &mut buf).await.body {
            Some(Body::Paired(p)) => p.device_secret,
            other => panic!("expected Paired, got {other:?}"),
        };
        assert!(matches!(
            read_env(&mut sock, &mut buf).await.body,
            Some(Body::Heartbeat(_))
        ));
        (port, sock, secret, buf)
    }

    async fn reconnect(port: u16, id: &str, secret: &[u8]) -> (TcpStream, Vec<u8>) {
        let mut sock = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let hello = env(
            1,
            Body::Hello(Hello {
                device_id: id.into(),
                display_name: "Fake".into(),
                one_time_token: vec![],
                rpc_port: 50052,
                device_secret: secret.to_vec(),
            }),
        );
        sock.write_all(&framing::encode(&hello)).await.unwrap();
        let mut buf = Vec::new();
        assert!(
            matches!(
                read_env(&mut sock, &mut buf).await.body,
                Some(Body::Heartbeat(_))
            ),
            "reconnect with secret must be accepted"
        );
        (sock, buf)
    }

    /// Pretend a run with `plan_id` is live and this device holds a plan for it.
    fn fake_run(st: &AppState, id: &str, plan_id: &str, role: &str) {
        {
            let mut r = st.run.write().unwrap();
            r.status = "ready".into();
            r.plan_id = Some(plan_id.into());
        }
        let mut d = st.devices.write().unwrap();
        let dev = d.get_mut(id).unwrap();
        dev.has_plan = true;
        dev.role = role.into();
        dev.last_plan = Some(Plan {
            plan_id: plan_id.into(),
            placements: vec![meshcore::proto::Placement {
                device_id: id.into(),
                is_host: role == "host",
                used: true,
                ..Default::default()
            }],
            ..Default::default()
        });
    }

    async fn wait_status(st: &AppState, want: &str) -> bool {
        for _ in 0..50 {
            if run_status(st) == want {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        false
    }

    async fn wait_offline(st: &AppState, id: &str) -> bool {
        for _ in 0..50 {
            if !st
                .devices
                .read()
                .unwrap()
                .get(id)
                .map(|d| d.online)
                .unwrap_or(true)
            {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        false
    }

    #[tokio::test]
    async fn reconnect_resend_then_drop_again_ends_the_run() {
        // Round-5 #2: after a re-send the device must be a full member again (has_plan AND role),
        // so its next disconnect ends the run.
        let st = test_state();
        let (port, sock, secret, _buf) = pair(&st, "p6").await;
        drop(sock);
        assert!(wait_offline(&st, "p6").await);
        fake_run(&st, "p6", "plan-D", "worker");
        {
            // What the owning cleanup leaves behind when the socket dropped before the push.
            let mut d = st.devices.write().unwrap();
            let dev = d.get_mut("p6").unwrap();
            dev.has_plan = false;
            dev.role = "idle".into();
        }
        let (mut sock2, mut buf2) = reconnect(port, "p6", &secret).await;
        let e = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_env(&mut sock2, &mut buf2),
        )
        .await
        .unwrap();
        assert!(matches!(e.body, Some(Body::Plan(_))));
        assert_eq!(st.devices.read().unwrap().get("p6").unwrap().role, "worker");
        drop(sock2);
        assert!(
            wait_status(&st, "error").await,
            "a member dropping must end the run"
        );
    }

    #[tokio::test]
    async fn reconnect_that_wins_while_cleanup_waits_keeps_the_run() {
        // Round-4 #4: cleanup blocks on run_lock; the phone reconnects meanwhile; when cleanup
        // resumes it must notice it no longer owns the device and leave the run alone.
        let st = test_state();
        let (port, sock, secret, _buf) = pair(&st, "p7").await;
        fake_run(&st, "p7", "plan-E", "worker");
        let held = st.run_lock.lock().await; // an api call is busy
        drop(sock);
        assert!(wait_offline(&st, "p7").await); // cleanup ran its first half, now waits on run_lock
        let (_sock2, _buf2) = reconnect(port, "p7", &secret).await;
        assert!(st.devices.read().unwrap().get("p7").unwrap().online);
        drop(held);
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        assert_eq!(
            run_status(&st),
            "ready",
            "cleanup must not end a run the device rejoined"
        );
        assert!(st.devices.read().unwrap().get("p7").unwrap().has_plan);
    }

    #[tokio::test]
    async fn stop_withdraws_a_resent_plan_and_nothing_is_resent_after_a_stop() {
        // Round-6 #2: a phone that reconnected and was handed the current plan must receive the
        // stop; after the stop a reconnect gets nothing.
        let st = test_state();
        let (port, sock, secret, _buf) = pair(&st, "p9").await;
        drop(sock);
        assert!(wait_offline(&st, "p9").await);
        fake_run(&st, "p9", "plan-G", "worker");
        st.devices.write().unwrap().get_mut("p9").unwrap().has_plan = false;
        let (mut sock2, mut buf2) = reconnect(port, "p9", &secret).await;
        let e = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_env(&mut sock2, &mut buf2),
        )
        .await
        .unwrap();
        assert!(matches!(&e.body, Some(Body::Plan(p)) if p.plan_id == "plan-G"));
        crate::supervisor::stop(&st).await;
        let e = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_env(&mut sock2, &mut buf2),
        )
        .await
        .expect("the stop must reach the reconnected member");
        assert!(matches!(&e.body, Some(Body::Plan(p)) if p.plan_id == "stop"));
        drop(sock2);
        assert!(wait_offline(&st, "p9").await);
        let (mut sock3, mut buf3) = reconnect(port, "p9", &secret).await;
        let r = tokio::time::timeout(
            std::time::Duration::from_millis(800),
            read_env(&mut sock3, &mut buf3),
        )
        .await;
        assert!(
            r.is_err(),
            "no plan may be re-sent after the run was stopped, got {r:?}"
        );
        assert!(!st.devices.read().unwrap().get("p9").unwrap().has_plan);
    }

    #[tokio::test]
    async fn preauth_pool_is_per_ip_and_released() {
        let st = test_state();
        let (port, _sock, _secret, _buf) = pair(&st, "p10").await; // authenticated: holds no pre-auth slot
        let mut silent = Vec::new();
        for _ in 0..MAX_PREAUTH_PER_IP {
            silent.push(TcpStream::connect(("127.0.0.1", port)).await.unwrap());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let mut extra = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let mut b = Vec::new();
        let closed = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            try_read_env(&mut extra, &mut b),
        )
        .await
        .expect("the 5th pre-auth socket from one address must be dropped at once");
        assert!(closed.is_none());
        // A different source address still gets in while 127.0.0.1 is saturated (per-IP, not global).
        let other = tokio::net::TcpSocket::new_v4().unwrap();
        other.bind("127.0.0.2:0".parse().unwrap()).unwrap();
        let mut from_other = other.connect(([127, 0, 0, 1], port).into()).await.unwrap();
        let r = tokio::time::timeout(
            std::time::Duration::from_millis(800),
            try_read_env(&mut from_other, &mut b),
        )
        .await;
        assert!(
            r.is_err(),
            "a second address must not be blocked by the first one's pre-auth sockets, got {r:?}"
        );
        drop(silent); // slots are released when the unpaired sockets close
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let mut again = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let r = tokio::time::timeout(
            std::time::Duration::from_millis(800),
            try_read_env(&mut again, &mut b),
        )
        .await;
        assert!(r.is_err(), "after release a new pre-auth socket must be accepted (kept open until the Hello deadline), got {r:?}");
    }

    #[tokio::test]
    async fn forgetting_a_member_ends_the_run() {
        let st = test_state();
        let (_port, mut sock, _secret, mut buf) = pair(&st, "p8").await;
        fake_run(&st, "p8", "plan-F", "host");
        crate::supervisor::forget_device(&st, "p8").await;
        assert_eq!(run_status(&st), "error");
        assert!(st
            .run
            .read()
            .unwrap()
            .error
            .as_deref()
            .unwrap_or("")
            .contains("forgotten"));
        let e = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            try_read_env(&mut sock, &mut buf),
        )
        .await
        .unwrap();
        assert!(matches!(e.map(|e| e.body), Some(Some(Body::Bye(_)))));
        assert!(!st.pairing.lock().unwrap().verify("p8", &_secret));
    }

    fn run_status(st: &AppState) -> String {
        st.run.read().unwrap().status.clone()
    }

    #[tokio::test]
    async fn plan_pushed_while_offline_arrives_on_reconnect_and_marks_has_plan() {
        let st = test_state();
        let (port, sock, secret, _buf) = pair(&st, "p1").await;
        drop(sock);
        assert!(wait_offline(&st, "p1").await);
        // The run starts while the phone is between connections: the plan is recorded for it.
        fake_run(&st, "p1", "plan-A", "worker");
        st.devices.write().unwrap().get_mut("p1").unwrap().has_plan = false; // cleanup cleared it
        let (mut sock2, mut buf2) = reconnect(port, "p1", &secret).await;
        let e = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            read_env(&mut sock2, &mut buf2),
        )
        .await
        .expect("plan must be re-sent on reconnect");
        match e.body {
            Some(Body::Plan(p)) => assert_eq!(p.plan_id, "plan-A"),
            other => panic!("expected the plan, got {other:?}"),
        }
        assert!(
            st.devices.read().unwrap().get("p1").unwrap().has_plan,
            "a re-sent plan must mark the device as holding it (round-4 #4)"
        );
    }

    #[tokio::test]
    async fn failure_report_ends_only_the_run_it_belongs_to() {
        let st = test_state();
        let (_port, mut sock, _secret, _buf) = pair(&st, "p2").await;
        fake_run(&st, "p2", "plan-B", "host");
        let stale = env(
            5,
            Body::JobResult(JobResult {
                job_id: "host".into(),
                ok: false,
                output: b"download cancelled".to_vec(),
                plan_id: "plan-OLD".into(),
                ..Default::default()
            }),
        );
        sock.write_all(&framing::encode(&stale)).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        assert_eq!(
            run_status(&st),
            "ready",
            "a report for a superseded plan must not touch the current run (round-4 #1)"
        );
        let current = env(
            6,
            Body::JobResult(JobResult {
                job_id: "host".into(),
                ok: false,
                output: b"llama-server exited (1)".to_vec(),
                plan_id: "plan-B".into(),
                ..Default::default()
            }),
        );
        sock.write_all(&framing::encode(&current)).await.unwrap();
        for _ in 0..20 {
            if run_status(&st) == "error" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(run_status(&st), "error");
        assert!(st
            .run
            .read()
            .unwrap()
            .error
            .as_deref()
            .unwrap_or("")
            .contains("llama-server exited"));
    }

    #[tokio::test]
    async fn idle_device_disconnecting_leaves_the_run_alone_but_a_member_ends_it() {
        let st = test_state();
        let (port, sock, secret, _buf) = pair(&st, "p3").await;
        st.run.write().unwrap().status = "ready".into();
        st.run.write().unwrap().plan_id = Some("plan-C".into());
        drop(sock); // p3 is idle: not part of the run
        assert!(wait_offline(&st, "p3").await);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert_eq!(run_status(&st), "ready");
        assert!(!st.devices.read().unwrap().get("p3").unwrap().online);
        let (sock2, _b) = reconnect(port, "p3", &secret).await;
        fake_run(&st, "p3", "plan-C", "worker");
        drop(sock2); // now it is a member: the run must end
        for _ in 0..20 {
            if run_status(&st) == "error" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(run_status(&st), "error");
        assert!(st
            .run
            .read()
            .unwrap()
            .error
            .as_deref()
            .unwrap_or("")
            .contains("disconnected"));
    }

    #[tokio::test]
    async fn forgetting_a_device_says_bye_and_closes_its_session() {
        let st = test_state();
        let (_port, mut sock, _secret, mut buf) = pair(&st, "p4").await;
        crate::supervisor::forget_device(&st, "p4").await;
        let e = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            try_read_env(&mut sock, &mut buf),
        )
        .await
        .expect("Bye must arrive promptly");
        assert!(matches!(e.map(|e| e.body), Some(Some(Body::Bye(_)))));
        let closed = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            try_read_env(&mut sock, &mut buf),
        )
        .await
        .expect("socket must close after Bye");
        assert!(closed.is_none());
        assert!(st.devices.read().unwrap().get("p4").is_none());
    }

    #[tokio::test]
    async fn silent_link_is_dropped_and_oversized_preauth_frame_refused() {
        let st = test_state();
        st.silence_limit_ms.store(3_000, Ordering::Relaxed);
        let (port, mut sock, _secret, mut buf) = pair(&st, "p5").await;
        // Say nothing: the first heartbeat tick (10 s) sees > 3 s of silence and drops us.
        let r = tokio::time::timeout(std::time::Duration::from_secs(15), async {
            loop {
                match try_read_env(&mut sock, &mut buf).await {
                    None => return None,
                    Some(e) if matches!(e.body, Some(Body::Heartbeat(_))) => continue,
                    Some(e) => return Some(e),
                }
            }
        })
        .await
        .expect("silent link must be dropped within 15 s");
        assert!(r.is_none(), "expected the socket to be closed, got {r:?}");
        assert!(!st.devices.read().unwrap().get("p5").unwrap().online);
        // Pre-auth: a 1 MiB frame header is refused before any body is read.
        let mut s2 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        s2.write_all(&(1u32 << 20).to_be_bytes()).await.unwrap();
        let mut b2 = Vec::new();
        let closed = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            try_read_env(&mut s2, &mut b2),
        )
        .await
        .expect("oversized pre-auth frame must be refused promptly");
        assert!(closed.is_none());
        // Pre-auth: no Hello within 5 s is dropped.
        let mut s3 = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let mut b3 = Vec::new();
        let closed = tokio::time::timeout(
            std::time::Duration::from_secs(8),
            try_read_env(&mut s3, &mut b3),
        )
        .await
        .expect("silent pre-auth socket must be dropped within the Hello deadline");
        assert!(closed.is_none());
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
        // Handshake heartbeat + ticks at 10 s and 20 s: a 20 s interval would only give 2.
        assert!(
            heartbeats >= 3,
            "expected >= 3 heartbeats in 25 s, got {heartbeats}"
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
        // The superseded socket is closed by the coordinator (round-4 #8) ...
        let old = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            try_read_env(&mut sock, &mut buf),
        )
        .await
        .expect("old socket must be closed after the takeover");
        assert!(old.is_none(), "expected EOF on the superseded socket");
        drop(sock);
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        // ... and its cleanup must not mark the reconnected device offline.
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
