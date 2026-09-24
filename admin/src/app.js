/* MeshAI admin — three pages (Devices · Run · Chat), plain language. Vanilla JS, no build step. */
(() => {
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => [...r.querySelectorAll(s)];
  let TOKEN = ""; try { const q = new URLSearchParams(location.search).get("token"); if (q) localStorage.setItem("meshai-token", q); TOKEN = localStorage.getItem("meshai-token") || ""; } catch {}
  const authHeaders = () => TOKEN ? { "x-mesh-token": TOKEN } : {};
  const api = async (path, opts = {}) => {
    const r = await fetch(path, { ...opts, headers: { "content-type": "application/json", ...authHeaders(), ...(opts.headers || {}) } });
    const t = await r.text();
    let j; try { j = JSON.parse(t); } catch { j = { raw: t }; }
    if (!r.ok) throw new Error(j.error || j.raw || r.statusText);
    return j;
  };
  const gb = b => (b / 1e9).toFixed(1) + " GB";
  const fmt = (n, d = 1) => (n == null || isNaN(n)) ? "—" : Number(n).toFixed(d);
  const esc = s => String(s ?? "").replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  const toast = m => { const t = $("#toast"); t.textContent = m; t.hidden = false; clearTimeout(t._t); t._t = setTimeout(() => t.hidden = true, 3200); };

  /* ---- icons ---- */
  const ICONS = {
    phone: '<rect x="7" y="2" width="10" height="20" rx="2"/><path d="M11 18h2"/>',
    play: '<path d="M6 4v16l14-8z"/>', stop: '<rect x="6" y="6" width="12" height="12" rx="2"/>',
    chat: '<path d="M21 12a8 8 0 0 1-8 8H8l-5 3 1.5-4.5A8 8 0 1 1 21 12z"/>',
    sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
    qr: '<path d="M3 3h6v6H3zM15 3h6v6h-6zM3 15h6v6H3zM15 15h2v2h-2zM19 15h2v2h-2zM15 19h2v2h-2zM19 19h2v2h-2z"/>',
    download: '<path d="M12 3v12m0 0 4-4m-4 4-4-4M4 17v3h16v-3"/>', refresh: '<path d="M21 12a9 9 0 1 1-3-6.7M21 3v6h-6"/>',
    send: '<path d="M22 2 11 13M22 2l-7 20-4-9-9-4z"/>', cloud: '<path d="M7 18a4 4 0 0 1-.5-8 6 6 0 0 1 11.4-1.5A4.5 4.5 0 0 1 17.5 18z"/>',
    attach: '<path d="M21 12.5 12.5 21a5 5 0 0 1-7-7l9-9a3.5 3.5 0 0 1 5 5l-9 9a2 2 0 0 1-3-3l8-8"/>',
    check: '<path d="M4 12l5 5L20 6"/>', x: '<path d="M6 6l12 12M18 6 6 18"/>',
  };
  const svg = (n, s = 16) => `<svg viewBox="0 0 24 24" width="${s}" height="${s}" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">${ICONS[n] || ""}</svg>`;
  const drawIcons = (root = document) => $$("i[data-i]", root).forEach(i => { i.innerHTML = svg(i.dataset.i); });
  drawIcons();
  const DEV_ICON = {
    phone: '<svg class="dv-icon" viewBox="0 0 34 34"><rect x="9" y="3" width="16" height="28" rx="3"/><circle class="fill" cx="17" cy="27" r="1.4"/><path d="M14 6h6"/></svg>',
    laptop: '<svg class="dv-icon" viewBox="0 0 34 34"><rect x="6" y="7" width="22" height="15" rx="2"/><path d="M3 26h28M12 26l1-4h8l1 4"/></svg>',
    sim: '<svg class="dv-icon" viewBox="0 0 34 34"><rect x="9" y="3" width="16" height="28" rx="3" stroke-dasharray="3 2"/><path d="M14 6h6"/></svg>',
  };
  const kindOf = d => { const k = (d.kind || "").toLowerCase(); return k.includes("sim") ? "sim" : k.includes("laptop") ? "laptop" : "phone"; };

  /* ---- theme / pages / advanced ---- */
  const root = document.documentElement;
  try { root.dataset.theme = localStorage.getItem("meshai-theme") || "light"; } catch {}
  $("#theme").onclick = () => { root.dataset.theme = root.dataset.theme === "dark" ? "light" : "dark"; try { localStorage.setItem("meshai-theme", root.dataset.theme); } catch {} };
  let page = "devices";
  const show = p => { page = p; $$("#pages button").forEach(b => b.classList.toggle("active", b.dataset.page === p)); $$(".page").forEach(s => s.hidden = s.id !== "page-" + p); try { localStorage.setItem("meshai-page", p); } catch {} render(); };
  $$("#pages button").forEach(b => b.onclick = () => show(b.dataset.page));
  const adv = $("#adv");
  try { adv.checked = localStorage.getItem("meshai-adv") === "1"; } catch {}
  const applyAdv = () => { $$(".advanced").forEach(a => a.hidden = !adv.checked); try { localStorage.setItem("meshai-adv", adv.checked ? "1" : "0"); } catch {} render(); };
  adv.onchange = applyAdv;

  /* ---- state ---- */
  let S = { devices: [], models: [], run: {}, plan: null, downloads: [] };
  let CAT = [], RUNS = [], OFFER = null, USB = { adb: false, devices: [] }, usbBusy = false;
  let PREVIEW = null, PREVIEW_KEY = "", PREVIEW_ARGS = null, LAST_CHAT = null, CHATTING = false, SELECTED = null;
  const devById = id => S.devices.find(d => d.id === id);
  const devName = id => (devById(id) || { name: id }).name;
  const phones = () => S.devices.filter(d => !d.is_local && d.online);
  const online = () => S.devices.filter(d => d.online);
  const pooled = () => online().reduce((a, d) => a + (d.usable_bytes || 0), 0);
  const catOf = file => CAT.find(c => c.file === file);
  const isVision = file => !!(catOf(file) && catOf(file).mmproj);
  const isProjector = file => /^mmproj-/i.test(file) || (catOf(file) && catOf(file).role === "projector");

  async function poll() {
    try { S = await api("/api/state"); if (adv.checked && page === "run") RUNS = await api("/api/runs"); render(); }
    catch (e) { $("#now-title").textContent = "meshd is not reachable"; $("#status-text").textContent = "offline"; }
  }
  async function pollUsb() { try { USB = await api("/api/usb"); } catch { USB = { adb: false, devices: [] }; } if (page === "devices") renderUsb(); }

  /* ---- top bar: what is happening right now ---- */
  const fmtEta = s => s < 60 ? `${Math.round(s)} s` : s < 3600 ? `${Math.round(s / 60)} min` : `${(s / 3600).toFixed(1)} h`;
  const activeDownload = () => (S.downloads || []).find(d => !d.done && d.total > 0);
  const readyKey = m => "meshai-ready-" + m;
  const usualReady = m => { try { const v = +localStorage.getItem(readyKey(m)); return v > 0 ? v : null; } catch { return null; } };
  function nowLine() {
    const r = S.run || {}; const p = phones()[0]; const pr = p && p.progress; const fresh = pr && (Date.now() - pr.ms) < 120000;
    const model = (r.model || (S.plan && S.plan.model) || "").replace(/\.gguf$/, "");
    const dl = activeDownload();
    if (dl && r.status !== "ready" && r.status !== "loading") {
      const secs = Math.max(1, ((S.now_ms || Date.now()) - dl.started_ms) / 1000); const rate = dl.bytes / secs; const left = rate > 0 ? (dl.total - dl.bytes) / rate : null;
      return [`Downloading ${dl.file.replace(/\.gguf$/, "")}`, `${(dl.bytes / 1e6).toFixed(0)} of ${(dl.total / 1e6).toFixed(0)} MB · ${(rate / 1e6).toFixed(1)} MB/s · ${left != null ? "about " + fmtEta(left) + " left" : ""}`, dl.bytes / dl.total];
    }
    if (r.status === "ready" && r.ready_ms && r.started_ms) { try { localStorage.setItem(readyKey(r.model), String(r.ready_ms - r.started_ms)); } catch {} }
    const elapsed = r.started_ms ? ((S.now_ms || Date.now()) - r.started_ms) / 1000 : 0;
    const usual = usualReady(r.model || "");
    if (CHATTING) return ["Answering your question", `${model} · ${S.plan ? (S.plan.mode === "Single" ? "on " + devName(S.plan.host_id) : "split across " + S.plan.placements.filter(x => x.role !== "Rejected").length + " devices") : ""}`, null];
    if (fresh && pr.job === "download" && pr.fraction < 1) { const mdl = S.models.find(m => m.file === r.model); const total = mdl ? mdl.info.file_bytes : 0; const done = total * pr.fraction; const rate = r.started_ms ? done / Math.max(1, elapsed) : 0; return [`Sending ${model} to ${p.name}`, `${Math.round(pr.fraction * 100)} % · ${total ? `${(done / 1e6).toFixed(0)} of ${(total / 1e6).toFixed(0)} MB` : ""}${rate > 0 ? ` · ${(rate / 1e6).toFixed(1)} MB/s · about ${fmtEta((total - done) / rate)} left` : ""}`, pr.fraction]; }
    switch (r.status) {
      case "starting": return [`Planning ${model}`, `deciding which device holds which layers · ${elapsed.toFixed(0)} s`, null];
      case "loading": return [`Loading ${model} into memory`, `${elapsed.toFixed(0)} s so far${usual ? ` · usually about ${fmtEta(usual / 1000)} for this model` : " · the first time takes longest"} · ${S.plan && S.plan.mode !== "Single" ? "helper starts its engine, then the host loads" : "the host loads the model"}`, usual ? Math.min(0.95, elapsed * 1000 / usual) : null];
      case "ready": return [`Ready: ${model}`, `answers come from ${devName(r.host_id)}${S.plan && S.plan.mode !== "Single" ? " with help from " + S.plan.placements.filter(x => x.role === "Worker").map(x => x.name).join(", ") : ""} · ready in ${r.ready_ms && r.started_ms ? ((r.ready_ms - r.started_ms) / 1000).toFixed(1) : "?"} s`, null];
      case "error": return [`Stopped: ${r.error || "error"}`, "press Run to try again", null];
      default: return phones().length ? ["Devices connected — pick a model and press Run", `${online().length} devices online · ${gb(pooled())} for models`, null] : ["Connect your phone to begin", "USB cable or QR, on the Devices page", null];
    }
  }
  function deviceChecklist() {
    const r = S.run || {}; if (!r.status || r.status === "idle") return "";
    const plan = S.plan; if (!plan) return "";
    return plan.placements.filter(x => x.role !== "Rejected").map(x => {
      const d = devById(x.device_id) || {}; const pr = d.progress; const fresh = pr && (Date.now() - pr.ms) < 180000;
      let state, text;
      if (x.role === "Host") { state = r.status === "ready" ? "done" : (r.status === "error" ? "" : "busy"); text = r.status === "ready" ? "model loaded, answering" : fresh && pr.job === "download" && pr.fraction < 1 ? `receiving model ${Math.round(pr.fraction * 100)} %` : "loading the model"; }
      else { const ready = (fresh && pr.job === "worker") || (d.worker_ready_plan && d.worker_ready_plan === r.plan_id); state = ready ? "done" : (r.status === "error" ? "" : "busy"); text = ready ? "helper ready" : "starting its engine"; }
      return `<span class="${state}"><b></b>${esc(d.name || x.name)}: ${text}</span>`;
    }).join("");
  }
  function renderNow() {
    const [h, sub, frac] = nowLine();
    $("#now-title").textContent = h; $("#now-sub").textContent = sub;
    $("#now-dev").innerHTML = deviceChecklist();
    const bar = $("#now-bar"); bar.hidden = frac == null; if (frac != null) bar.firstElementChild.style.width = (frac * 100).toFixed(0) + "%";
    const r = S.run || {};
    $("#status-pill").className = "status " + (r.status || "idle");
    $("#status-text").textContent = ({ idle: "idle", starting: "starting", loading: "loading", ready: "ready", error: "error", stopping: "stopping" })[r.status] || r.status || "idle";
    $("#stop-btn").hidden = !r.status || r.status === "idle";
    $("#stop-btn2").disabled = !r.status || r.status === "idle";
    $("#mesh-id").textContent = S.mesh_id || "—";
  }

  /* ---- page 1: devices ---- */
  function renderUsb() {
    const el = $("#usb"); if (!el) return;
    const paired = phones().length > 0;
    $("#usb-sub").textContent = !USB.adb ? "adb not found on this laptop" : USB.devices.length ? `${USB.devices.length} phone${USB.devices.length > 1 ? "s" : ""} on USB` : "no phone with USB debugging found";
    el.innerHTML = USB.devices.map(d => `<div class="u">${DEV_ICON.phone}<div class="grow"><div class="u-name">${esc(d.model)}</div><div class="u-serial">${esc(d.serial)}${paired ? " · connected" : ""}</div></div><button class="btn primary" data-usb="${esc(d.serial)}" ${usbBusy ? "disabled" : ""}>${svg("phone")} ${paired ? "Re-pair" : "Pair over USB"}</button></div>`).join("");
    $$("[data-usb]", el).forEach(b => b.onclick = async () => { usbBusy = true; renderUsb(); try { await api("/api/usb/pair", { method: "POST", body: JSON.stringify({ serial: b.dataset.usb }) }); toast("Now tap Join on the phone"); } catch (e) { toast(e.message); } usbBusy = false; renderUsb(); });
  }
  const permIcon = ok => ok ? `<b class="ok">${svg("check", 12)}</b>` : `<b class="no">${svg("x", 12)}</b>`;
  function perms(d) {
    const list = ((d.profile || {}).permissions || []);
    const extra = d.is_local ? [["phone on USB (adb)", USB.adb && USB.devices.length > 0, USB.adb ? (USB.devices.length ? USB.devices[0].model : "none plugged in") : "adb not installed"]] : [];
    if (!list.length) return [["permissions", false, "not reported yet"], ...extra];
    return list.map(s => { const i = s.indexOf(":"); const k = i > 0 ? s.slice(0, i) : s; const v = i > 0 ? s.slice(i + 1).trim() : ""; const ok = !/not granted|off$|default|missing/i.test(v); return [k, ok, v]; }).concat(extra);
  }
  function coreBars(d, big) {
    const tel = d.telemetry || {}; const loads = tel.core_loads || []; const caps = ((d.profile || {}).cores || []).map(c => c.capacity);
    if (!loads.length) return "";
    return `<div class="cores ${big ? "big" : ""}" title="each bar = one CPU core · taller = running faster right now">${loads.map((l, i) => `<i class="${(caps[i] || 1024) >= 1000 ? "prime" : (caps[i] || 1024) >= 500 ? "big" : "small"}" style="height:${Math.max(8, Math.round(l * 100))}%"></i>`).join("")}</div>`;
  }
  function liveCompute(d) {
    const tel = d.telemetry || {}; const pr = d.profile || {};
    const total = pr.total_bytes || 0; const free = tel.avail_bytes || 0; const usable = d.usable_bytes || 0;
    const cores = coreBars(d, true) ? `${coreBars(d, true)}<div class="muted xs">cores at work · taller = faster right now</div>` : `<div class="muted xs">${d.is_local ? "per-core load is not reported for the laptop yet" : "waiting for the first sample…"}</div>`;
    const mem = total ? `<div class="membar"><i class="used" style="width:${(100 * (total - free) / total).toFixed(0)}%"></i><i class="keep" style="width:${(100 * Math.max(0, free - usable) / total).toFixed(0)}%"></i><i class="free" style="width:${(100 * usable / total).toFixed(0)}%"></i></div><div class="legend"><span><b class="used"></b>other apps ${gb(total - free)}</span><span><b class="keep"></b>kept safe</span><span><b class="free"></b>for models ${gb(usable)}</span></div>` : "";
    const chips = [];
    if (!d.is_local && tel.thermal_headroom != null) chips.push(`${tel.thermal_headroom < 0.5 ? "cool" : tel.thermal_headroom < 0.85 ? "warm" : "hot"} · headroom ${fmt(tel.thermal_headroom, 2)}`);
    if (!d.is_local && tel.battery_pct) chips.push(`battery ${fmt(tel.battery_pct, 0)}%${tel.charging ? " ⚡" : ""}`);
    if (!d.is_local && tel.rtt_ms_p50 > 0) chips.push(`link ${fmt(tel.rtt_ms_p50, 1)} ms · worst ${fmt(tel.rtt_ms_p95, 0)} ms`);
    if (tel.held_bytes) chips.push(`model in memory ${gb(tel.held_bytes)}`);
    return `${cores}${mem}<div class="chips">${chips.map(c => `<span class="chip">${esc(c)}</span>`).join("")}</div>`;
  }
  function specRows(d) {
    const pr = d.profile || {}; const cores = (pr.cores || []);
    const big = cores.filter(c => c.capacity >= 500).length, prime = cores.filter(c => c.capacity >= 1000).length;
    const tier = { 0: "tier A · host or helper", 1: "tier A · host or helper", 2: "tier B · helper", 3: "unsupported" }[pr.tier];
    return [["chip", (pr.soc || "—").split(" · ")[0]], ["cores", cores.length ? `${cores.length}${d.is_local ? "" : ` (${prime} prime · ${big - prime} big · ${cores.length - big} small)`}` : "—"], ["memory", pr.total_bytes ? gb(pr.total_bytes) : "—"], ["system", (pr.os || "—").split(" (")[0]], ["capability", d.is_local ? "host or helper" : (tier || "—")], ["role now", d.role && d.role !== "idle" ? d.role : (d.online ? "waiting" : "offline")]];
  }
  function deviceCard(d) {
    const p = S.plan ? S.plan.placements.find(x => x.device_id === d.id) : null; const used = p && p.role !== "Rejected";
    return `<div class="dv ${used ? (p.role === "Host" ? "host" : "worker") : ""} ${d.online ? "" : "offline"}">
      <div class="dv-top">${DEV_ICON[kindOf(d)]}<div class="grow"><div class="dv-name">${esc(d.name)}</div><div class="dv-role">${d.is_local ? "this laptop" : "phone"} · ${d.online ? "online" : "offline"}${used ? ` · ${p.role === "Host" ? "runs the model" : "helper"}` : ""}</div></div>${used ? `<span class="sticker">${p.role === "Host" ? "host" : "helper"}</span>` : ""}</div>
      <div class="dv-sec"><div class="dv-h">Specs</div><div class="kv2">${specRows(d).map(([k, v]) => `<span class="k">${esc(k)}</span><span class="v">${esc(v)}</span>`).join("")}</div></div>
      <div class="dv-sec"><div class="dv-h">Permissions &amp; access</div><div class="perms">${perms(d).map(([k, ok, v]) => `<div class="perm">${permIcon(ok)}<span>${esc(k)}</span><em>${esc(v)}</em></div>`).join("")}</div></div>
      <div class="dv-sec"><div class="dv-h">Compute right now</div>${liveCompute(d)}</div>
      ${used ? `<div class="dv-sec"><div class="dv-h">Its share of the model</div><div class="mn-layers">layers ${p.layer_start}–${Math.max(p.layer_start, p.layer_end - 1)} · ${p.layer_end - p.layer_start} of ${nLayers()} · ${gb(p.bytes)}</div>${layerBar(p)}</div>` : ""}
      ${adv.checked ? `<div class="dv-sec row"><label class="sm muted">usable cap (GB) <input class="sm" style="width:80px" type="number" step="0.5" value="${d.usable_override_bytes ? (d.usable_override_bytes / 1e9).toFixed(1) : ""}" placeholder="auto" data-limit="${esc(d.id)}"></label>${d.is_local ? "" : `<button class="btn sm ghost" data-forget="${esc(d.id)}">Forget</button>`}</div>` : ""}
    </div>`;
  }
  function renderDevices() {
    const el = $("#devices2");
    const list = S.devices.slice().sort((a, b) => (a.is_local ? -1 : 1) - (b.is_local ? -1 : 1) || (b.online - a.online));
    el.innerHTML = list.map(deviceCard).join("") + (phones().length ? "" : `<div class="dv placeholder">${DEV_ICON.phone}<div class="dv-name">Your phone</div><div class="muted sm">appears here once it joins — specs, permissions and live compute</div></div>`);
    const p = phones()[0]; const tel = p && p.telemetry || {};
    $("#link-sub").textContent = p ? `linked ${p.addr === "127.0.0.1" ? "over the USB cable" : "over " + p.addr} · round trip ${fmt(tel.rtt_ms_p50, 1)} ms · ${gb(pooled())} for models across ${online().length} devices` : "";
    $("#connect-card").classList.toggle("done", phones().length > 0);
    $$("[data-limit]", el).forEach(i => i.onchange = async () => { await api(`/api/devices/${i.dataset.limit}/limit`, { method: "POST", body: JSON.stringify({ usable_gb: i.value ? +i.value : null }) }); toast("cap updated"); poll(); });
    $$("[data-forget]", el).forEach(b => b.onclick = async () => { await api(`/api/devices/${b.dataset.forget}`, { method: "DELETE" }); poll(); });
    if (OFFER) { $("#qr").innerHTML = OFFER.svg; $("#qr-text").innerHTML = `The phone will try: ${(OFFER.links || []).map(l => `<span class="tag">${esc(l.kind)}</span> <span class="mono">${esc(l.ip)}</span>`).join(" → ")}. One-time code, valid 10 minutes.`; }
  }

  /* ---- page 2: run ---- */
  const nLayers = () => { const plan = S.plan || PREVIEW; const m = plan && S.models.find(x => x.file === plan.model); return m ? m.info.n_layer : (plan ? Math.max(...plan.placements.map(p => p.layer_end)) : 0); };
  const layerBar = p => { const nl = nLayers() || 1; return `<div class="mn-bar"><i style="left:${(p.layer_start / nl * 100).toFixed(1)}%;width:${((p.layer_end - p.layer_start) / nl * 100).toFixed(1)}%"></i></div>`; };
  function fitLabel(m) {
    const laptop = online().find(d => d.is_local); const need = m.info.file_bytes * 1.15 + 0.2e9;
    if (laptop && laptop.usable_bytes >= need) return ["fits the laptop alone", "ok"];
    if (pooled() >= need) return ["needs laptop + phone", "split"];
    return ["too big for what is free now", "no"];
  }
  const PROVIDER = { qwen: ["Q", "Qwen (Alibaba)"], openai: ["O", "OpenAI"], google: ["G", "Google"], meta: ["M", "Meta"], mistral: ["Mi", "Mistral"], huggingface: ["HF", "Hugging Face"] };
  function providerOf(m) { const c = catOf(m.file); if (c && PROVIDER[c.provider]) return PROVIDER[c.provider]; const n = (m.info.name || m.file).toLowerCase(); if (n.includes("qwen")) return PROVIDER.qwen; if (n.includes("gpt") || n.includes("oss")) return PROVIDER.openai; if (n.includes("gemma")) return PROVIDER.google; if (n.includes("llama")) return PROVIDER.meta; if (n.includes("mistral")) return PROVIDER.mistral; return ["M", "model"]; }
  const paramsOf = m => { const s = (m.info.name || m.file); const a = s.match(/(\d+(?:\.\d+)?)B-A(\d+(?:\.\d+)?)B/i); if (a) return `${a[1]}B total · ${a[2]}B active`; const b = s.match(/(\d+(?:\.\d+)?)\s?B\b/i); return b ? `${b[1]}B parameters` : "—"; };
  const ktok = n => n >= 1000 ? `${Math.round(n / 1000)}k tokens` : `${n} tokens`;
  function modelCard(m) {
    const run = S.run || {}; const [label, cls] = fitLabel(m); const on = m.file === SELECTED; const vis = isVision(m.file); const [mark, prov] = providerOf(m);
    const arch = (m.info.arch || "").toLowerCase(); const tools = /qwen3|gpt-oss|llama|mistral|gemma/.test(arch) || /qwen3|gpt-oss/i.test(m.info.name || ""); const thinks = /qwen3|gpt-oss/i.test(arch + (m.info.name || ""));
    const c = catOf(m.file);
    return `<button class="mc ${on ? "on" : ""} ${cls}" data-model="${esc(m.file)}" title="${esc(c ? c.note : m.file)}">
      <div class="mc-head"><div class="mc-mark">${esc(mark)}</div><div><div class="mc-name">${esc(m.info.name || m.file)}</div><div class="mc-meta">${esc(prov)}${c && c.role ? " · " + esc(c.role) : ""}</div></div></div>
      <div class="mc-spec"><b>size</b><span>${paramsOf(m)} · ${esc(m.info.quant_label)} · ${gb(m.info.file_bytes)} file</span><b>shape</b><span>${m.info.n_layer} layers${m.info.n_expert > 1 ? ` · ${m.info.n_expert} experts, ${m.info.n_expert_used} used` : ""}</span><b>memory</b><span>remembers up to ${ktok(m.info.n_ctx_train)} (${(m.info.kv_bytes_per_token / 1024).toFixed(0)} KB per token)</span></div>
      <div class="mc-io"><i class="yes">text in</i><i class="${vis ? "img" : "no"}">${vis ? "images in" : "no images"}</i><i class="yes">text out</i><i class="${tools ? "yes" : "no"}">${tools ? "tool calls" : "no tools"}</i><i class="${thinks ? "yes" : "no"}">${thinks ? "can think" : "direct"}</i></div>
      <div class="mc-fit">${label}</div>${run.model === m.file && run.status === "ready" ? `<div class="mc-live">running</div>` : ""}
    </button>`;
  }
  function renderModels() {
    const el = $("#model-cards"); const run = S.run || {};
    const runnable = S.models.filter(m => !isProjector(m.file));
    if (!SELECTED && runnable.length) SELECTED = (run.model && runnable.find(m => m.file === run.model)) ? run.model : runnable[0].file;
    el.innerHTML = runnable.map(modelCard).join("") || `<div class="muted">No model files yet — turn on Advanced to download one.</div>`;
    $$("[data-model]", el).forEach(b => b.onclick = () => { SELECTED = b.dataset.model; renderModels(); preview(); });
    const hs = $("#qs-host"); const hcur = hs.value;
    hs.innerHTML = `<option value="">auto</option>` + online().filter(d => kindOf(d) !== "sim").map(d => `<option value="${esc(d.id)}">${esc(d.name)}</option>`).join("");
    if ([...hs.options].some(o => o.value === hcur)) hs.value = hcur;
  }
  async function preview() {
    if (!SELECTED) return;
    const key = `${SELECTED}|${$("#qs-ctx").value}|${$("#qs-host").value}|${online().length}`; if (key === PREVIEW_KEY) return; PREVIEW_KEY = key;
    try { const r = await api("/api/plan", { method: "POST", body: JSON.stringify({ model: SELECTED, n_ctx: +$("#qs-ctx").value, host: $("#qs-host").value || null }) }); PREVIEW = r.plan; PREVIEW_ARGS = r.args; $("#qs-result").className = "plan-mini"; $("#qs-result").textContent = r.plan.summary; }
    catch (e) { PREVIEW = null; $("#qs-result").className = "plan-mini bad"; $("#qs-result").textContent = e.message; }
    renderMesh();
  }
  $("#qs-ctx").onchange = () => { PREVIEW_KEY = ""; preview(); }; $("#qs-host").onchange = () => { PREVIEW_KEY = ""; preview(); };
  function deviceStatus(d, p) {
    const r = S.run || {}; const pr = d.progress; const fresh = pr && (Date.now() - pr.ms) < 120000;
    if (fresh && pr.job === "download" && pr.fraction < 1) return { text: "receiving the model from the laptop", pct: pr.fraction };
    if (!p) return { text: d.online ? "online · not in this plan" : "offline", pct: null };
    if (p.role === "Rejected") return { text: "not used: " + (p.reason || "").replace(/^rejected: /, ""), pct: null };
    if (!S.plan) return { text: "will " + (p.role === "Host" ? "run the model" : "help with its layers"), pct: null };
    if (p.role === "Host") return { text: r.status === "ready" ? "runs the model and answers" : r.status === "loading" ? "loading the model" : "will run the model", pct: null };
    if (fresh && pr.job === "worker") return { text: "compute engine ready", pct: null };
    return { text: r.status === "ready" ? "computing its layers for the host" : "starting its compute engine", pct: null };
  }
  function renderMesh() {
    const plan = S.plan || PREVIEW; const live = !!S.plan;
    const mm = $("#mesh-model"); const mi = plan ? (S.models.find(m => m.file === plan.model) || {}) : {}; const info = mi.info || {};
    mm.style.display = plan ? "" : "none";
    mm.innerHTML = plan ? `<span class="mm-name">${esc(info.name || plan.model)}</span><span class="mm-meta">${esc(info.quant_label || "")} · ${info.n_layer || "?"} layers · remembers ${plan.n_ctx} tokens · needs ${gb(plan.total_needed_bytes)}</span>${live ? "" : `<span class="tag">preview</span>`}` : "";
    const used = plan ? plan.placements.filter(p => p.role !== "Rejected").sort((a, b) => a.layer_start - b.layer_start) : [];
    const nl = nLayers() || 1;
    $("#stack").innerHTML = used.length ? `<div class="stackbar">${used.map(p => `<i class="${p.role === "Host" ? "host" : "worker"}" style="flex:${Math.max(1, p.layer_end - p.layer_start)}" title="${esc(p.name)}">${p.layer_end - p.layer_start}</i>`).join("")}</div><div class="legend">${used.map(p => `<span><b class="${p.role === "Host" ? "host" : "worker"}"></b>${esc(p.name)} · layers ${p.layer_start}–${Math.max(p.layer_start, p.layer_end - 1)}</span>`).join("")}</div>` : "";
    const rows = plan ? plan.placements.map(p => ({ d: devById(p.device_id) || { id: p.device_id, name: p.name, online: true, kind: "phone" }, p })) : online().map(d => ({ d, p: null }));
    $("#mesh").innerHTML = rows.map(({ d, p }) => { const usedP = p && p.role !== "Rejected"; const st = deviceStatus(d, p);
      return `<div class="mnode ${usedP ? (p.role === "Host" ? "host" : "worker") : "unused"}"><div class="mn-top">${DEV_ICON[kindOf(d)]}<div><div class="mn-name">${esc(d.name)}</div><div class="mn-role">${usedP ? (p.role === "Host" ? "host · runs the model" : "helper · computes layers") : "not needed"}</div></div></div>${coreBars(d)}${usedP ? `<div class="mn-layers">layers ${p.layer_start}–${Math.max(p.layer_start, p.layer_end - 1)} · ${p.layer_end - p.layer_start} of ${nl}${p.role === "Host" ? " + embeddings/output" : ""} · ${gb(p.bytes)}</div>${layerBar(p)}` : ""}<div class="mn-status">${esc(st.text)}${st.pct != null ? ` <span class="pct">${Math.round(st.pct * 100)}%</span><div class="pbar"><i style="width:${(st.pct * 100).toFixed(0)}%"></i></div>` : ""}</div></div>`; }).join("") || '<div class="muted">connect a device first</div>';
    $("#mesh-sub").textContent = plan ? (live ? (plan.mode === "Single" ? "one device" : `${used.length} devices`) : "preview") : "no plan";
    const args = (S.run && S.run.args && S.run.args.length) ? S.run.args : (PREVIEW_ARGS ? [PREVIEW_ARGS.program, ...PREVIEW_ARGS.args] : []);
    $("#plan-args").textContent = args.join(" ");
    $("#placements").innerHTML = plan ? plan.placements.map(p => `<div class="placement ${p.role.toLowerCase()}"><b>${esc(p.name)}</b><span class="tag ${esc(p.role.toLowerCase())}">${esc(p.role)}</span><span class="muted">${esc(p.reason)}</span></div>`).join("") : "";
  }
  function renderFeed() {
    const r = S.run || {}; const lines = (r.log_tail || []).slice(-30).reverse();
    const cls = l => /error|failed|✗|exited/i.test(l) ? "err" : /ready|listening|paired|credits|serving/i.test(l) ? "ok" : /download|fetch|push|plan|host:/i.test(l) ? "io" : "";
    const extra = S.devices.filter(d => d.progress && (Date.now() - d.progress.ms) < 120000).map(d => `<div class="line io"><b></b><span>${esc(d.name)}: ${esc(d.progress.job)} ${Math.round(d.progress.fraction * 100)}%</span></div>`).join("");
    $("#feed").innerHTML = extra + lines.map(l => `<div class="line ${cls(l)}"><b></b><span>${esc(l.length > 140 ? l.slice(0, 140) + "…" : l)}</span></div>`).join("") || '<div class="muted sm">nothing yet</div>';
    $("#log").textContent = (r.log_tail || []).join("\n") + (r.error ? "\nERROR: " + r.error : "");
  }
  function renderAdvancedModels() {
    if (!adv.checked) return;
    const onDisk = new Set(S.models.map(m => m.file)); const dl = f => (S.downloads || []).find(d => d.file === f);
    $("#catalog").innerHTML = CAT.map(c => { const d = dl(c.file); const have = onDisk.has(c.file); const pct = d && d.total ? Math.round(100 * d.bytes / d.total) : 0;
      return `<div class="model"><div class="model-h"><div class="mark">${esc((c.provider || "M")[0].toUpperCase())}</div><div><div class="m-name">${esc(c.name)}</div><div class="m-sub">${esc(c.file)}</div></div></div><div class="m-role">${esc(c.role)} · ${gb(c.bytes)}${c.layers ? ` · ${c.layers} layers` : ""}</div><div class="muted sm">${esc(c.note)}</div>${d && !d.done ? `<div class="progress"><i style="width:${pct}%"></i></div><div class="muted xs mono">${gb(d.bytes)} / ${gb(d.total)} · ${pct}%</div>` : ""}<div class="row">${have ? `<span class="tag">on disk</span>` : `<button class="btn sm" data-dl="${c.id}" ${d && !d.done ? "disabled" : ""}>${svg("download")} ${d && !d.done ? "Downloading…" : "Download"}</button>`}</div></div>`; }).join("");
    $$("[data-dl]", $("#catalog")).forEach(b => b.onclick = async () => { try { await api("/api/models/download", { method: "POST", body: JSON.stringify({ id: b.dataset.dl }) }); toast("download started"); poll(); } catch (e) { toast(e.message); } });
    $("#disk").style.setProperty("--cols", "2fr 1fr 1fr 1fr 1fr");
    $("#disk").innerHTML = `<div class="tr th"><span>file</span><span>arch</span><span>quant</span><span class="num">size</span><span class="num">layers</span></div>` + S.models.map(m => `<div class="tr"><span class="mono">${esc(m.file)}</span><span>${esc(m.info.arch)}</span><span>${esc(m.info.quant_label)}</span><span class="num">${gb(m.info.file_bytes)}</span><span class="num">${m.info.n_layer}</span></div>`).join("");
  }
  function renderRuns() {
    if (!adv.checked || page !== "run") return;
    const by = {}; for (const r of RUNS) { if (!r.tokens_out) continue; const k = `${r.model} · ${r.mode} · ${r.devices} dev · host ${devName(r.host)}`; (by[k] ||= []).push(r.tps); }
    const agg = Object.entries(by).map(([k, v]) => [k, v.reduce((a, b) => a + b, 0) / v.length, v.length]).sort((a, b) => b[1] - a[1]); const max = Math.max(1, ...agg.map(a => a[1]));
    $("#runs-bars").innerHTML = agg.map(([k, v, n]) => `<div class="bar"><span title="${esc(k)}">${esc(k)} <span class="muted xs">×${n}</span></span><div class="track"><i style="width:${100 * v / max}%"></i></div><span class="num">${fmt(v, 1)} tok/s</span></div>`).join("") || '<div class="muted sm">no requests yet</div>';
    const ok = RUNS.filter(r => r.ok && r.tokens_out > 0); const avg = f => ok.length ? ok.reduce((a, r) => a + f(r), 0) / ok.length : NaN;
    $("#runs-stats").innerHTML = [["Requests", RUNS.length, ""], ["Avg decode", fmt(avg(r => r.tps), 1), "tok/s"], ["Avg TTFT", fmt(avg(r => r.ttft_ms), 0), "ms"]].map(([k, v, u]) => `<div class="stat"><div class="k">${k}</div><div class="v">${v}<small>${u}</small></div></div>`).join("");
    $("#runs-table").style.setProperty("--cols", "1.2fr 2fr 1fr 1fr .7fr .7fr .7fr");
    $("#runs-table").innerHTML = `<div class="tr th"><span>time</span><span>model</span><span>mode</span><span>host</span><span class="num">TTFT</span><span class="num">tok</span><span class="num">tok/s</span></div>` + RUNS.slice().reverse().slice(0, 100).map(r => `<div class="tr"><span class="mono xs">${new Date(r.ts_ms).toLocaleTimeString()}</span><span>${esc(r.model)}</span><span>${esc(r.mode)}</span><span>${esc(devName(r.host))}</span><span class="num">${r.ttft_ms} ms</span><span class="num">${r.tokens_out}</span><span class="num">${fmt(r.tps, 1)}</span></div>`).join("");
  }

  /* ---- page 3: chat ---- */
  const CANDO = [
    ["Summarise this", "Summarise the attached text in five bullet points.", "text"],
    ["Explain simply", "Explain like I am 12: how does a phone and a laptop share one AI model?", ""],
    ["Write code", "Write a Python function that parses a CSV file and prints the average of a numeric column.", ""],
    ["Analyse data", "Here is my data. Find the three most interesting patterns and give numbers.", "text"],
    ["Translate", "Translate this into Hindi, Telugu and Spanish: “The meeting moves to Thursday at 4 pm.”", ""],
    ["Extract fields", "Extract every date, amount and name from the attached document as a JSON list.", "text"],
    ["Describe this image", "Describe what is in this image in detail.", "image"],
    ["Read a screenshot", "Read the text in this screenshot and list the action items.", "image"],
    ["Plan my day", "Make a realistic schedule for a study day with three 90-minute focus blocks.", ""],
    ["Brainstorm", "Give me ten unusual product ideas that combine a phone and a laptop.", ""],
  ];
  function renderPower() {
    const r = S.run || {}; const p = phones()[0]; const lap = online().find(d => d.is_local);
    const vis = r.model && isVision(r.model);
    const cards = [
      ["Model", r.status === "ready" ? r.model.replace(/\.gguf$/, "") : "none running", r.status === "ready" ? (vis ? "text + images" : "text only") : "go to Run and press Run", r.status === "ready" ? "accent" : ""],
      ["Memory for models", gb(pooled()), `${online().length} device${online().length === 1 ? "" : "s"} online`, ""],
      ["Laptop", lap ? `${(lap.profile.cores || []).length} cores` : "—", lap ? gb(lap.usable_bytes) + " free for models" : "", S.plan && S.plan.host_id === "local" ? "host" : ""],
      ["Phone", p ? `${(p.profile && p.profile.cores || []).length} cores` : "not connected", p ? `${gb(p.usable_bytes)} free · link ${fmt((p.telemetry || {}).rtt_ms_p50, 1)} ms` : "", p && S.plan && S.plan.placements.some(x => x.device_id === p.id && x.role !== "Rejected") ? (S.plan.host_id === p.id ? "host" : "worker") : ""],
      ["Last answer", LAST_CHAT ? `${fmt(LAST_CHAT.tps, 1)} tok/s` : "—", LAST_CHAT ? `${fmt(LAST_CHAT.ttft, 0)} ms to first word` : "", ""],
    ];
    $("#power").innerHTML = cards.map(([k, v, s, cls]) => `<div class="pw ${cls}"><div class="k">${esc(k)}</div><div class="v">${esc(v)}</div><div class="s">${esc(s)}</div></div>`).join("") + (p ? `<div class="pw"><div class="k">Phone cores now</div>${coreBars(p) || '<div class="s">waiting…</div>'}</div>` : "");
    $("#cando").innerHTML = CANDO.map(([t, q, need]) => { const dis = (need === "image" && !vis); return `<button data-q="${esc(q)}" data-need="${need}" class="${dis ? "needs" : ""}" title="${dis ? "needs a vision model (Run → Qwen2.5-VL)" : q}">${esc(t)}</button>`; }).join("");
    $$("#cando button").forEach(b => b.onclick = () => { $("#chat-input").value = b.dataset.q; if (b.dataset.need) { $("#chat-file").click(); } else { $("#chat-input").focus(); } });
    $("#chat-model").textContent = r.status === "ready" ? `${r.model.replace(/\.gguf$/, "")} on ${devName(r.host_id)}${vis ? " · can see images" : ""}` : "start a model first (Run page)";
    const used = S.plan ? S.plan.placements.filter(x => x.role !== "Rejected") : [];
    $("#who").innerHTML = used.length ? used.map(x => { const d = devById(x.device_id) || { kind: "phone" }; return `<div class="w ${x.role === "Host" ? "host" : "worker"}">${DEV_ICON[kindOf(d)]}<div>${esc(x.name)}<br><span class="muted xs">${x.role === "Host" ? "writes the answer" : "computes layers " + x.layer_start + "–" + Math.max(x.layer_start, x.layer_end - 1)}</span></div></div>`; }).join("") : '<div class="muted sm">no model running</div>';
  }

  function render() { renderNow(); if (page === "devices") { renderDevices(); renderUsb(); } else if (page === "run") { renderModels(); renderMesh(); renderFeed(); renderAdvancedModels(); renderRuns(); } else { renderPower(); } }

  /* ---- actions ---- */
  const stop = async () => { try { await api("/api/stop", { method: "POST" }); toast("stopped"); poll(); } catch (e) { toast(e.message); } };
  $("#stop-btn").onclick = stop; $("#stop-btn2").onclick = stop;
  $("#qs-run").onclick = async () => {
    if (!SELECTED) return toast("pick a model");
    try { await api("/api/run", { method: "POST", body: JSON.stringify({ model: SELECTED, n_ctx: +$("#qs-ctx").value, host: $("#qs-host").value || null }) }); toast("starting…"); poll(); }
    catch (e) { $("#qs-result").className = "plan-mini bad"; $("#qs-result").textContent = e.message; toast(e.message); }
  };
  $("#offer-btn").onclick = async () => { try { OFFER = await api("/api/pair/offer", { method: "POST" }); renderDevices(); } catch (e) { toast(e.message); } };
  $("#sim-btn").onclick = async () => { try { await api("/api/sim/workers", { method: "POST", body: JSON.stringify({ n: +$("#sim-n").value, usable_gb: +$("#sim-gb").value, spawn: $("#sim-spawn").checked }) }); toast(`simulated ${$("#sim-n").value} phone(s)`); poll(); } catch (e) { toast(e.message); } };
  $("#dl-btn").onclick = async () => { try { await api("/api/models/download", { method: "POST", body: JSON.stringify({ url: $("#dl-url").value, file: $("#dl-file").value }) }); toast("download started"); poll(); } catch (e) { toast(e.message); } };
  $("#rescan").onclick = async () => { await api("/api/models/rescan", { method: "POST" }); poll(); };

  /* ---- chat with attachments ---- */
  const history = []; let ATT = []; // {name, kind:"image"|"text", data}
  const addMsg = (role, html) => { const d = document.createElement("div"); d.className = "msg " + role; d.innerHTML = html; $("#chat").appendChild(d); $("#chat").scrollTop = 1e9; return d; };
  $("#chat-clear").onclick = () => { history.length = 0; $("#chat").innerHTML = ""; };
  $("#chat-input").addEventListener("keydown", e => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); $("#chat-form").requestSubmit(); } });
  $("#chat-file").onchange = async e => {
    for (const f of e.target.files) {
      if (f.type.startsWith("image/")) { const data = await new Promise(res => { const r = new FileReader(); r.onload = () => res(r.result); r.readAsDataURL(f); }); ATT.push({ name: f.name, kind: "image", data }); }
      else { const text = await f.text(); ATT.push({ name: f.name, kind: "text", data: text.slice(0, 60000) }); }
    }
    e.target.value = ""; renderAtt();
  };
  function renderAtt() { $("#attach-list").innerHTML = ATT.map((a, i) => `<span class="att">${a.kind === "image" ? `<img src="${a.data}" alt="">` : "📄"} ${esc(a.name)}${a.kind === "text" ? ` · ${(a.data.length / 1000).toFixed(0)} k chars` : ""}<button data-rm="${i}">✕</button></span>`).join(""); $$("[data-rm]").forEach(b => b.onclick = () => { ATT.splice(+b.dataset.rm, 1); renderAtt(); }); }
  async function streamChat(url, key, model, messages, onDelta) {
    const t0 = performance.now(); let ttft = null; let n = 0; let text = ""; let think = "";
    const r = await fetch(url + "/chat/completions", { method: "POST", headers: { "content-type": "application/json", ...(key ? { authorization: "Bearer " + key } : (url === "/v1" ? authHeaders() : {})) }, body: JSON.stringify({ model, messages, stream: true, stream_options: { include_usage: true } }) });
    if (!r.ok) throw new Error(await r.text());
    const rd = r.body.getReader(); const dec = new TextDecoder(); let buf = "";
    while (true) {
      const { value, done } = await rd.read(); if (done) break;
      buf += dec.decode(value, { stream: true });
      let i; while ((i = buf.indexOf("\n")) >= 0) {
        const line = buf.slice(0, i).trim(); buf = buf.slice(i + 1);
        if (!line.startsWith("data:")) continue; const j = line.slice(5).trim(); if (j === "[DONE]") continue;
        try { const v = JSON.parse(j); const delta = v.choices?.[0]?.delta || {}; const rc = delta.reasoning_content, d = delta.content;
          if (rc) { if (ttft == null) ttft = performance.now() - t0; n++; think += rc; onDelta(`<think>${think}</think>${text}`); }
          if (d) { if (ttft == null) ttft = performance.now() - t0; n++; text += d; onDelta(think ? `<think>${think}</think>${text}` : text); }
          if (v.usage?.completion_tokens) n = v.usage.completion_tokens; } catch {}
      }
    }
    const total = performance.now() - t0; return { text: think ? `<think>${think}</think>${text}` : text, ttft: ttft ?? total, total, tokens: n, tps: n * 1000 / Math.max(1, total - (ttft ?? 0)) };
  }
  const renderThink = t => { const m = t.match(/^<think>([\s\S]*?)(<\/think>|$)([\s\S]*)$/); return m ? `<div class="think">${esc(m[1].trim())}</div>${esc(m[3].trim())}` : esc(t); };
  let lastPrompt = "";
  $("#chat-form").onsubmit = async e => {
    e.preventDefault(); const q = $("#chat-input").value.trim(); if (!q && !ATT.length) return;
    if ((S.run || {}).status !== "ready") return toast("start a model first (Run page)");
    const images = ATT.filter(a => a.kind === "image"), texts = ATT.filter(a => a.kind === "text");
    if (images.length && !isVision(S.run.model)) return toast("this model cannot see images — run Qwen2.5-VL (Run page, Advanced → download)");
    const userText = (texts.length ? texts.map(t => `--- ${t.name} ---\n${t.data}\n--- end ---`).join("\n\n") + "\n\n" : "") + (q || "Describe the attachment.");
    const content = images.length ? [{ type: "text", text: userText }, ...images.map(im => ({ type: "image_url", image_url: { url: im.data } }))] : userText;
    $("#chat-input").value = ""; lastPrompt = userText;
    addMsg("user", `${images.map(im => `<img src="${im.data}" alt="">`).join("")}${texts.map(t => `<div class="file">📄 ${esc(t.name)}</div>`).join("")}${esc(q || "Describe the attachment.")}`);
    history.push({ role: "user", content }); ATT = []; renderAtt();
    const m = addMsg("assistant", "…"); $("#chat-send").disabled = true; CHATTING = true; renderNow();
    try {
      const res = await streamChat("/v1", null, S.run.model, history, t => { m.innerHTML = renderThink(t); $("#chat").scrollTop = 1e9; });
      history.push({ role: "assistant", content: res.text }); LAST_CHAT = res;
      m.innerHTML = renderThink(res.text) + `<div class="meta">${fmt(res.ttft, 0)} ms to first word · ${fmt(res.tps, 1)} tok/s · ${res.tokens} tok · ${fmt(res.total / 1000, 1)} s</div>`;
      $("#s-ttft").innerHTML = fmt(res.ttft, 0) + "<small>ms</small>"; $("#s-tps").innerHTML = fmt(res.tps, 1) + "<small>tok/s</small>"; $("#s-tok").textContent = res.tokens; $("#s-total").innerHTML = fmt(res.total / 1000, 1) + "<small>s</small>";
    } catch (err) { m.textContent = "✗ " + err.message; }
    CHATTING = false; $("#chat-send").disabled = false; render();
  };
  $("#ref-send").onclick = async () => {
    const url = $("#ref-url").value.trim(); const key = $("#ref-key").value.trim(); const model = $("#ref-model").value.trim();
    if (!url || !model || !lastPrompt) return toast("set endpoint + model and send a prompt locally first");
    try { $("#ref-out").textContent = "…"; const res = await streamChat(url, key, model, [{ role: "user", content: lastPrompt }], () => {}); $("#ref-out").textContent = `cloud: ${fmt(res.ttft, 0)} ms · ${fmt(res.tps, 1)} tok/s`; } catch (e) { $("#ref-out").textContent = "cloud: " + e.message; }
  };

  /* ---- boot ---- */
  (async () => {
    try { CAT = await api("/api/catalog"); } catch {}
    try { const p = localStorage.getItem("meshai-page"); if (["run", "devices", "chat"].includes(p)) page = p; } catch {}
    await poll(); await pollUsb(); applyAdv(); show(page); preview();
    setInterval(poll, 2000); setInterval(pollUsb, 5000);
  })();
})();
