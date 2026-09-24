/* MeshAI admin panel — vanilla JS, no build step. Talks to meshd's /api and /v1. */
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
  const esc = s => String(s).replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  const toast = m => { const t = $("#toast"); t.textContent = m; t.hidden = false; clearTimeout(t._t); t._t = setTimeout(() => t.hidden = true, 3200); };

  /* ---- icons (inline, monochrome) ---- */
  const ICONS = {
    grid: '<path d="M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z"/>',
    phone: '<rect x="7" y="2" width="10" height="20" rx="2"/><path d="M11 18h2"/>',
    box: '<path d="M21 8 12 3 3 8v8l9 5 9-5zM3 8l9 5 9-5M12 13v8"/>',
    layers: '<path d="m12 2 9 5-9 5-9-5zM3 12l9 5 9-5M3 17l9 5 9-5"/>',
    chat: '<path d="M21 12a8 8 0 0 1-8 8H8l-5 3 1.5-4.5A8 8 0 1 1 21 12z"/>',
    chart: '<path d="M4 20V10M10 20V4M16 20v-7M22 20H2"/>',
    sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
    stop: '<rect x="6" y="6" width="12" height="12" rx="2"/>',
    play: '<path d="M6 4v16l14-8z"/>',
    qr: '<path d="M3 3h6v6H3zM15 3h6v6h-6zM3 15h6v6H3zM15 15h2v2h-2zM19 15h2v2h-2zM15 19h2v2h-2zM19 19h2v2h-2z"/>',
    download: '<path d="M12 3v12m0 0 4-4m-4 4-4-4M4 17v3h16v-3"/>',
    refresh: '<path d="M21 12a9 9 0 1 1-3-6.7M21 3v6h-6"/>',
    send: '<path d="M22 2 11 13M22 2l-7 20-4-9-9-4z"/>',
    cloud: '<path d="M7 18a4 4 0 0 1-.5-8 6 6 0 0 1 11.4-1.5A4.5 4.5 0 0 1 17.5 18z"/>',
  };
  const drawIcons = (root = document) => $$("i[data-i]", root).forEach(i => {
    i.innerHTML = `<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${ICONS[i.dataset.i] || ""}</svg>`;
  });
  drawIcons();

  /* ---- theme ---- */
  const root = document.documentElement;
  try { root.dataset.theme = localStorage.getItem("meshai-theme") || "dark"; } catch {}
  $("#theme").onclick = () => { root.dataset.theme = root.dataset.theme === "dark" ? "light" : "dark"; try { localStorage.setItem("meshai-theme", root.dataset.theme); } catch {} };

  /* ---- nav ---- */
  let view = "overview";
  const show = v => { view = v; $$("#nav button").forEach(b => b.classList.toggle("active", b.dataset.view === v)); $$(".view").forEach(s => s.hidden = s.id !== "view-" + v); render(); };
  $$("#nav button").forEach(b => b.onclick = () => show(b.dataset.view));

  /* ---- state ---- */
  let S = { devices: [], models: [], run: {}, plan: null, downloads: [] };
  let CAT = [];
  let RUNS = [];
  let OFFER = null; // pairing offer lives only in this tab (never in /api/state)
  const providerMark = p => `<div class="mark ${p}">${{ qwen: "Q", openai: "O", anthropic: "A", huggingface: "HF" }[p] || "M"}</div>`;

  async function poll() {
    try {
      S = await api("/api/state");
      if (view === "runs") RUNS = await api("/api/runs");
      render();
    } catch (e) { $("#status-text").textContent = "meshd unreachable"; }
  }
  const devById = id => S.devices.find(d => d.id === id);
  const devName = id => (devById(id) || { name: id }).name;

  function render() {
    $("#mesh-id").textContent = S.mesh_id || "—";
    $("#models-dir").textContent = S.models_dir || "";
    const run = S.run || {};
    const pill = $("#status-pill"); pill.className = "status " + (run.status || "idle");
    $("#status-text").textContent = ({ idle: "idle", starting: "starting", loading: "loading model…", ready: "ready", error: "error", stopping: "stopping" })[run.status] || run.status;
    $("#host-label").textContent = run.model ? `${run.model} on ${devName(run.host_id)}` : "";
    $("#stop-btn").hidden = !run.status || run.status === "idle";
    $("#log").textContent = (run.log_tail || []).join("\n") + (run.error ? "\nERROR: " + run.error : "");
    $("#log").scrollTop = 1e9;

    // selects
    const ms = $("#qs-model"); const cur = ms.value;
    ms.innerHTML = S.models.map(m => `<option value="${esc(m.file)}">${esc(m.info.name || m.file)} · ${esc(m.info.quant_label)} · ${gb(m.info.file_bytes)} · ${m.info.n_layer}L</option>`).join("") || `<option value="">no models on disk — download one</option>`;
    if ([...ms.options].some(o => o.value === cur)) ms.value = cur;
    const hs = $("#qs-host"); const hcur = hs.value;
    hs.innerHTML = `<option value="">auto (fastest that fits)</option>` + S.devices.filter(d => d.online).map(d => `<option value="${esc(d.id)}">${esc(d.name)}</option>`).join("");
    if ([...hs.options].some(o => o.value === hcur)) hs.value = hcur;

    renderHero(); renderMesh(); renderTopology(); renderStats(); renderFeed(); renderDevices(); renderModels(); renderPlan(); renderRuns();
    $("#chat-model").textContent = run.status === "ready" ? `${run.model} · ${S.plan ? S.plan.mode : ""}` : "no model running";
    if (S.mirrored) { $("#chat-send").disabled = true; $("#qs-run").disabled = true; $("#qs-plan").disabled = true; $("#stop-btn").hidden = true; }
  }

  // ---- Mesh card: plain-language view of what the mesh is doing right now ----
  const ICON = {
    phone: '<svg class="mn-icon" viewBox="0 0 34 34"><rect x="9" y="3" width="16" height="28" rx="3"/><circle class="fill" cx="17" cy="27" r="1.4"/><path d="M14 6h6"/></svg>',
    laptop: '<svg class="mn-icon" viewBox="0 0 34 34"><rect x="6" y="7" width="22" height="15" rx="2"/><path d="M3 26h28M12 26l1-4h8l1 4"/></svg>',
    sim: '<svg class="mn-icon" viewBox="0 0 34 34"><rect x="9" y="3" width="16" height="28" rx="3" stroke-dasharray="3 2"/><path d="M14 6h6"/></svg>',
  };
  function kindOf(d) { const k = (d.kind || "").toLowerCase(); return k.includes("sim") ? "sim" : k.includes("laptop") ? "laptop" : "phone"; }
  function nowLine() {
    const r = S.run, plan = S.plan;
    const model = r.model || (plan && plan.model) || "";
    switch (r.status) {
      case "starting": return [`Planning and sending the plan for ${model}`, "each device is told which layers it will hold"];
      case "loading": return [`Devices are loading ${model}`, "a phone host first downloads the model from the laptop; workers start their compute process"];
      case "ready": return [`Serving answers: ${model}`, `OpenAI-compatible endpoint ${r.endpoint || ""} · ready in ${r.ready_ms && r.started_ms ? ((r.ready_ms - r.started_ms) / 1000).toFixed(1) : "?"} s`];
      case "error": return [`Run stopped: ${r.error || "error"}`, "fix the cause and press Run again"];
      default: return [S.devices.filter(d => d.online && !d.is_local).length ? "Ready to run — pick a model and press Run" : "Pair a phone (Devices → QR) and press Run to see the model split across devices", ""];
    }
  }
  function deviceStatus(d, p) {
    const r = S.run; const pr = d.progress; const fresh = pr && (Date.now() - pr.ms) < 120000;
    if (fresh && pr.job === "download" && pr.fraction < 1) return { text: `Downloading the model from the laptop`, pct: pr.fraction };
    if (fresh && pr.job === "download" && pr.fraction >= 1 && r.status !== "ready") return { text: "Model downloaded · starting llama.cpp", pct: null };
    if (!p) return { text: d.online ? "Online · not in this plan" : "Offline", pct: null };
    if (p.role === "Rejected") return { text: "Not used: " + (p.reason || ""), pct: null };
    if (p.role === "Host") return { text: r.status === "ready" ? "Runs the model and answers requests" : r.status === "loading" ? "Loading the model into memory" : "Will run the model", pct: null };
    if (fresh && pr.job === "worker") return { text: `Compute worker ready · ${pr.note || "listening"}`, pct: null };
    return { text: r.status === "ready" ? "Holding its layers · computing for the host" : "Starting its compute worker", pct: null };
  }
  function renderMesh() {
    const plan = S.plan; const [h, sub] = nowLine();
    $("#mesh-now").innerHTML = `${esc(h)}${sub ? `<span class="sub">${esc(sub)}</span>` : ""}`;
    const mm = $("#mesh-model");
    const mi = plan ? (S.models.find(m => m.file === plan.model) || {}) : {};
    const info = mi.info || {};
    mm.innerHTML = plan ? `<span class="mm-name">${esc(info.name || plan.model)}</span><span class="mm-meta">${esc(info.quant_label || "")} · ${info.n_layer || "?"} layers · context ${plan.n_ctx} · needs ${gb(plan.total_needed_bytes)}</span>` : "";
    mm.style.display = plan ? "" : "none";
    const nl = info.n_layer || (plan ? Math.max(...plan.placements.map(p => p.layer_end)) : 0);
    const devs = S.devices.slice().sort((a, b) => (b.online - a.online) || (a.is_local ? -1 : 1));
    $("#mesh").innerHTML = devs.map(d => {
      const p = plan ? plan.placements.find(x => x.device_id === d.id) : null;
      const role = p ? p.role : (d.role === "idle" ? "" : d.role);
      const used = p && p.role !== "Rejected";
      const st = deviceStatus(d, p);
      const n = used ? p.layer_end - p.layer_start : 0;
      const bar = used && nl ? `<div class="mn-bar"><i style="left:${(p.layer_start / nl * 100).toFixed(1)}%;width:${(n / nl * 100).toFixed(1)}%"></i></div>` : "";
      const layers = used ? `<div class="mn-layers">layers ${p.layer_start}–${Math.max(p.layer_start, p.layer_end - 1)} · ${n} of ${nl}${p.role === "Host" ? " + embeddings/output" : ""} · ${gb(p.bytes)}</div>` : "";
      return `<div class="mnode ${used ? (p.role === "Host" ? "host" : "worker") : "unused"} ${d.online ? "" : "offline"}">
        <div class="mn-top">${ICON[kindOf(d)]}<div><div class="mn-name">${esc(d.name)}</div><div class="mn-role">${esc(used ? (p.role === "Host" ? "host · runs the model" : "compute worker") : (role || "paired · idle"))}</div></div></div>
        <div class="mn-spec">${esc(specOf(d))}</div>
        ${coreBars(d)}
        ${layers}${bar}
        <div class="mn-status">${esc(st.text)}${st.pct != null ? ` <span class="pct">${Math.round(st.pct * 100)}%</span><div class="pbar"><i style="width:${(st.pct * 100).toFixed(0)}%"></i></div>` : ""}</div>
      </div>`;
    }).join("") || '<div class="muted">no devices</div>';
    $("#mesh-sub").textContent = plan ? plan.summary : `${S.devices.filter(d => d.online).length} device(s) online`;
  }
  function coreBars(d) {
    const tel = d.telemetry || {}; const loads = tel.core_loads || []; const caps = ((d.profile || {}).cores || []).map(c => c.capacity);
    if (!loads.length) return "";
    return `<div class="cores" title="each bar = one CPU core, height = current clock / max">${loads.map((l, i) => `<i class="${(caps[i] || 1024) >= 1000 ? "prime" : (caps[i] || 1024) >= 500 ? "big" : "small"}" style="height:${Math.max(6, Math.round(l * 100))}%"></i>`).join("")}</div>`;
  }
  function specOf(d) {
    const pr = d.profile || {}; const tel = d.telemetry || {};
    const parts = [];
    if (pr.soc) parts.push(pr.soc.split(" · ")[0]);
    const cores = (pr.cores || []).filter(c => c.allowed).length; if (cores) parts.push(cores + " cores");
    if (pr.total_bytes) parts.push(gb(pr.total_bytes) + " RAM");
    if (pr.os) parts.push(pr.os.split(" (")[0]);
    if (tel.avail_bytes) parts.push(gb(tel.avail_bytes) + " free");
    if (d.usable_override_bytes != null) parts.push("capped at " + gb(d.usable_override_bytes));
    return parts.join(" · ") || "no profile yet";
  }
  let LAST_CHAT = null;
  function renderHero() {
    const [h, sub] = nowLine();
    $("#hero-title").textContent = h; $("#hero-sub").textContent = sub || "MeshAI pools a phone's and a laptop's memory over your own link and serves one OpenAI-compatible endpoint.";
    const r = S.run || {};
    $("#act-stop").disabled = !r.status || r.status === "idle"; $("#act-run").disabled = !S.models.length;
    const online = S.devices.filter(d => d.online); const phones = online.filter(d => !d.is_local);
    const pooled = online.reduce((a, d) => a + (d.usable_bytes || 0), 0);
    const load = (r.ready_ms && r.started_ms) ? ((r.ready_ms - r.started_ms) / 1000).toFixed(1) : null;
    $("#kpis").innerHTML = [
      ["Devices online", `${online.length}<small>${phones.length} phone${phones.length === 1 ? "" : "s"}</small>`, ""],
      ["Pooled usable memory", gb(pooled), ""],
      ["Model", r.model ? esc(r.model.replace(/\.gguf$/, "")) : "—", r.status === "ready" ? "accent" : ""],
      ["Last answer", LAST_CHAT ? `${fmt(LAST_CHAT.tps, 1)}<small>tok/s · ${fmt(LAST_CHAT.ttft, 0)} ms TTFT</small>` : "—", ""],
      ["Ready in", load ? `${load}<small>s</small>` : "—", ""],
    ].map(([k, v, cls]) => `<div class="stat ${cls}"><div class="k">${k}</div><div class="v">${v}</div></div>`).join("");
  }
  function renderFeed() {
    const r = S.run || {}; const lines = (r.log_tail || []).slice(-40).reverse();
    const cls = l => /error|failed|✗|exited/i.test(l) ? "err" : /ready|listening|paired|credits|serving/i.test(l) ? "ok" : /download|fetch|push|plan|host:/i.test(l) ? "io" : "";
    const extra = S.devices.filter(d => d.progress && (Date.now() - d.progress.ms) < 120000).map(d => `<div class="line io"><b></b><span>${esc(d.name)}: ${esc(d.progress.job)} ${Math.round(d.progress.fraction * 100)}% ${esc(d.progress.note || "")}</span></div>`).join("");
    $("#feed").innerHTML = extra + lines.map(l => `<div class="line ${cls(l)}"><b></b><span>${esc(l.length > 160 ? l.slice(0, 160) + "…" : l)}</span></div>`).join("") || '<div class="muted sm">nothing yet — press Run</div>';
  }
  function renderTopology() {
    const plan = S.plan;
    const used = plan ? plan.placements.filter(p => p.role !== "Rejected") : [];
    const t = $("#topology");
    if (!used.length) {
      t.innerHTML = S.devices.map(d => nodeHtml(d, null)).join('<div class="link"></div>') || '<div class="muted">no devices</div>';
      $("#topo-sub").textContent = `${S.devices.filter(d => d.online).length} online · no plan`;
      return;
    }
    t.innerHTML = used.map(p => nodeHtml(devById(p.device_id) || { id: p.device_id, name: p.name, online: true }, p)).join('<div class="link"></div>');
    $("#topo-sub").textContent = plan.summary;
  }
  function nodeHtml(d, p) {
    const tel = d.telemetry || {};
    const usable = d.usable_bytes != null ? gb(d.usable_bytes) : "—";
    const role = p ? p.role.toLowerCase() : (d.role || "idle");
    return `<div class="node ${esc(role)} ${d.online ? "" : "offline"}">
      <div class="n-role">${esc(role)}${d.is_local ? " · local" : ""}</div>
      <div class="n-name" title="${esc(d.name)}">${esc(d.name)}</div>
      <div class="n-meta">${usable} usable${tel.rtt_ms_p50 ? ` · ${fmt(tel.rtt_ms_p50, 0)} ms` : ""}${tel.thermal_status ? ` · T${tel.thermal_status}` : ""}</div>
      ${p ? `<div class="n-layers">layers ${p.layer_start}–${Math.max(p.layer_start, p.layer_end - 1)} · ${gb(p.bytes)}</div>` : ""}
    </div>`;
  }
  function renderStats() {
    const last = (S.run.ready_ms && S.run.started_ms) ? ((S.run.ready_ms - S.run.started_ms) / 1000).toFixed(1) + " s" : "—";
    const online = S.devices.filter(d => d.online);
    const pooled = online.reduce((a, d) => a + (d.usable_bytes || 0), 0);
    $("#ov-stats").innerHTML = [
      ["Devices online", online.length, ""], ["Pooled usable memory", gb(pooled), ""], ["Model load time", last, ""],
    ].map(([k, v]) => `<div class="stat"><div class="k">${k}</div><div class="v">${v}</div></div>`).join("");
  }
  function renderDevices() {
    const el = $("#devices");
    el.innerHTML = S.devices.map(d => {
      const tel = d.telemetry || {}; const pr = d.profile || {};
      const total = pr.total_bytes || 0; const avail = tel.avail_bytes || 0;
      return `<div class="dev ${d.online ? "" : "offline"}">
        <div class="dev-h"><span class="name">${esc(d.name)}</span><span class="tag ${esc(d.role)}">${esc(d.role)}</span></div>
        <div class="kv">
          <span class="k">id</span><span class="v">${esc(d.id)}</span>
          <span class="k">kind</span><span class="v">${esc(d.kind)}${d.addr ? " · " + esc(d.addr) : ""}${d.is_local ? "" : " · rpc :" + d.rpc_port}</span>
          <span class="k">soc</span><span class="v">${esc(pr.soc || "—")}</span>
          <span class="k">os</span><span class="v">${esc(pr.os || "—")}</span>
          <span class="k">memory</span><span class="v">${avail ? gb(avail) + " free" : "—"}${total ? " / " + gb(total) : ""} → <b>${gb(d.usable_bytes || 0)} usable</b></span>
          <span class="k">link</span><span class="v">${tel.rtt_ms_p50 != null && tel.rtt_ms_p50 > 0 ? `${fmt(tel.rtt_ms_p50, 1)} / ${fmt(tel.rtt_ms_p95, 1)} ms p50/p95` : (d.is_local ? "local" : "—")}</span>
          <span class="k">thermal</span><span class="v">${tel.thermal_headroom ? "headroom " + fmt(tel.thermal_headroom, 2) : "—"}${tel.thermal_status ? " · status " + tel.thermal_status : ""}</span>
          <span class="k">battery</span><span class="v">${tel.battery_pct ? fmt(tel.battery_pct, 0) + "%" + (tel.charging ? " ⚡" : "") : "—"}${tel.current_ma ? " · " + tel.current_ma + " mA" : ""}</span>
          <span class="k">cpuset</span><span class="v">${esc(tel.cpus_allowed || "—")}</span>
          <span class="k">bench</span><span class="v">${d.bench_tps ? fmt(d.bench_tps, 1) + " tok/s" : "—"}</span>
        </div>
        ${total ? `<div class="meter"><i style="width:${Math.min(100, 100 * (d.usable_bytes || 0) / total)}%"></i></div>` : ""}
        ${coreBars(d)}
        <div class="row">
          <label class="sm muted">usable cap (GB) <input class="sm" style="width:80px" type="number" step="0.5" value="${d.usable_override_bytes ? (d.usable_override_bytes / 1e9).toFixed(1) : ""}" placeholder="auto" data-limit="${esc(d.id)}"></label>
          ${d.is_local ? `<button class="btn sm ghost" data-bench="${esc(d.id)}">Bench</button>` : `<button class="btn sm ghost" data-forget="${esc(d.id)}">Forget</button>`}
        </div>
      </div>`;
    }).join("") || '<div class="muted">no devices</div>';
    $$("[data-limit]", el).forEach(i => i.onchange = async () => { await api(`/api/devices/${i.dataset.limit}/limit`, { method: "POST", body: JSON.stringify({ usable_gb: i.value ? +i.value : null }) }); toast("cap updated"); poll(); });
    $$("[data-forget]", el).forEach(b => b.onclick = async () => { await api(`/api/devices/${b.dataset.forget}`, { method: "DELETE" }); poll(); });
    $$("[data-bench]", el).forEach(b => b.onclick = async () => { const m = $("#qs-model").value; if (!m) return toast("pick a model first"); b.disabled = true; try { const r = await api("/api/bench", { method: "POST", body: JSON.stringify({ model: m, threads: 4 }) }); toast(`laptop: ${fmt(r.prompt_tps, 0)} pp / ${fmt(r.decode_tps, 1)} tg tok/s`); } catch (e) { toast(e.message); } b.disabled = false; poll(); });
    if (OFFER) { $("#qr").innerHTML = OFFER.svg; $("#qr-text").innerHTML = `Scan with the MeshAI app. Coordinator <span class="mono">${esc(OFFER.host)}:${OFFER.port}</span>. Token is single-use, valid 10 min — shown only on this screen.`; }
    if (S.mirrored) { $("#offer-btn").disabled = true; $("#sim-btn").disabled = true; }
  }
  function renderModels() {
    const onDisk = new Set(S.models.map(m => m.file));
    const dl = f => (S.downloads || []).find(d => d.file === f);
    $("#catalog").innerHTML = CAT.map(c => {
      const d = dl(c.file); const have = onDisk.has(c.file);
      const pct = d && d.total ? Math.round(100 * d.bytes / d.total) : 0;
      return `<div class="model">
        <div class="model-h">${providerMark(c.provider)}<div><div class="m-name">${esc(c.name)}</div><div class="m-sub">${esc(c.file)}</div></div></div>
        <div class="m-role">${c.role} · ${gb(c.bytes)} · ${c.layers} layers</div>
        <div class="muted sm">${esc(c.note)}</div>
        ${d && !d.done ? `<div class="progress"><i style="width:${pct}%"></i></div><div class="muted xs mono">${gb(d.bytes)} / ${gb(d.total)} · ${pct}% · ${fmt(d.bytes / 1e6 / Math.max(1, (S.now_ms - d.started_ms) / 1000), 1)} MB/s</div>` : ""}
        ${d && d.done && d.error ? `<div class="muted xs">error: ${esc(d.error)}</div>` : ""}
        <div class="row">${have ? `<span class="tag">on disk</span>` : `<button class="btn sm" data-dl="${c.id}" ${d && !d.done ? "disabled" : ""}><i data-i="download"></i>${d && !d.done ? "Downloading…" : "Download"}</button>`}</div>
      </div>`;
    }).join("");
    drawIcons($("#catalog"));
    $$("[data-dl]", $("#catalog")).forEach(b => b.onclick = async () => { try { await api("/api/models/download", { method: "POST", body: JSON.stringify({ id: b.dataset.dl }) }); toast("download started"); poll(); } catch (e) { toast(e.message); } });
    $("#disk").style.setProperty("--cols", "2fr 1fr 1fr 1fr 1fr 1fr 1fr");
    $("#disk").innerHTML = `<div class="tr th"><span>file</span><span>arch</span><span>quant</span><span class="num">size</span><span class="num">layers</span><span class="num">ctx train</span><span class="num">KV/token</span></div>` +
      S.models.map(m => `<div class="tr"><span class="mono">${esc(m.file)}</span><span>${esc(m.info.arch)}${m.info.n_expert > 1 ? ` · MoE ${m.info.n_expert}/${m.info.n_expert_used}` : ""}</span><span>${esc(m.info.quant_label)}</span><span class="num">${gb(m.info.file_bytes)}</span><span class="num">${m.info.n_layer}</span><span class="num">${m.info.n_ctx_train}</span><span class="num">${(m.info.kv_bytes_per_token / 1024).toFixed(0)} KB</span></div>`).join("") || '<div class="muted sm">nothing on disk yet</div>';
  }
  let lastPlanArgs = null;
  function renderPlan() {
    const plan = S.plan || lastPlanPreview;
    if (!plan) { $("#plan-summary").textContent = "no plan — preview one from Overview"; $("#layer-map").innerHTML = ""; $("#placements").innerHTML = ""; $("#plan-args").textContent = ""; return; }
    $("#plan-summary").textContent = plan.summary;
    const used = plan.placements.filter(p => p.role !== "Rejected");
    const nl = Math.max(1, ...used.map(p => p.layer_end));
    $("#layer-map").innerHTML = used.map(p => `<div class="${p.role.toLowerCase()}" style="flex:${Math.max(1, p.layer_end - p.layer_start)}" title="${esc(p.name)}">${esc(p.name)} · ${p.layer_start}–${Math.max(p.layer_start, p.layer_end - 1)}</div>`).join("") + (used.length === 1 && used[0].layer_end === 0 ? "" : "");
    $("#placements").innerHTML = plan.placements.map(p => `<div class="placement ${p.role.toLowerCase()}"><b>${esc(p.name)}</b><span class="tag ${esc(p.role.toLowerCase())}">${esc(p.role)}</span><span class="muted">${esc(p.reason)}</span></div>`).join("");
    const args = (S.run && S.run.args && S.run.args.length) ? S.run.args : (lastPlanArgs ? [lastPlanArgs.program, ...lastPlanArgs.args] : []);
    $("#plan-args").textContent = args.join(" ");
  }
  let lastPlanPreview = null;

  /* ---- actions ---- */
  $("#qs-plan").onclick = async () => {
    try {
      const r = await api("/api/plan", { method: "POST", body: JSON.stringify({ model: $("#qs-model").value, n_ctx: +$("#qs-ctx").value, host: $("#qs-host").value || null }) });
      lastPlanPreview = r.plan; lastPlanArgs = r.args;
      $("#qs-result").textContent = r.plan.summary + "\n" + r.plan.placements.map(p => `• ${p.name}: ${p.reason}`).join("\n");
      renderPlan();
    } catch (e) { $("#qs-result").textContent = "✗ " + e.message; }
  };
  $("#qs-run").onclick = async () => {
    try {
      const r = await api("/api/run", { method: "POST", body: JSON.stringify({ model: $("#qs-model").value, n_ctx: +$("#qs-ctx").value, host: $("#qs-host").value || null }) });
      $("#qs-result").textContent = "▶ " + r.plan.summary; toast("starting…"); poll();
    } catch (e) { $("#qs-result").textContent = "✗ " + e.message; toast(e.message); }
  };
  $("#stop-btn").onclick = async () => { await api("/api/stop", { method: "POST" }); toast("stopped"); poll(); };
  $("#act-run").onclick = () => $("#qs-run").click();
  $("#act-stop").onclick = () => $("#stop-btn").click();
  $("#act-qr").onclick = () => { show("devices"); $("#offer-btn").click(); };
  $("#act-chat").onclick = () => show("chat");
  $("#offer-btn").onclick = async () => { try { OFFER = await api("/api/pair/offer", { method: "POST" }); renderDevices(); } catch (e) { toast(e.message); } };
  $("#sim-btn").onclick = async () => { try { const r = await api("/api/sim/workers", { method: "POST", body: JSON.stringify({ n: +$("#sim-n").value, usable_gb: +$("#sim-gb").value, spawn: $("#sim-spawn").checked }) }); toast(`simulated ${$("#sim-n").value} phone(s)${r.spawned_ports.length ? " with live RPC workers on " + r.spawned_ports.join(", ") : ""}`); poll(); } catch (e) { toast(e.message); } };
  $("#dl-btn").onclick = async () => { try { await api("/api/models/download", { method: "POST", body: JSON.stringify({ url: $("#dl-url").value, file: $("#dl-file").value }) }); toast("download started"); poll(); } catch (e) { toast(e.message); } };
  $("#rescan").onclick = async () => { await api("/api/models/rescan", { method: "POST" }); poll(); };

  /* ---- chat ---- */
  const history = [];
  const addMsg = (role, text) => { const d = document.createElement("div"); d.className = "msg " + role; d.textContent = text; $("#chat").appendChild(d); $("#chat").scrollTop = 1e9; return d; };
  $("#chat-clear").onclick = () => { history.length = 0; $("#chat").innerHTML = ""; };
  $("#chat-input").addEventListener("keydown", e => { if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); $("#chat-form").requestSubmit(); } });
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
        try {
          const v = JSON.parse(j); const delta = v.choices?.[0]?.delta || {};
          const r = delta.reasoning_content, d = delta.content;
          if (r) { if (ttft == null) ttft = performance.now() - t0; n++; think += r; onDelta(`<think>${think}</think>${text}`); }
          if (d) { if (ttft == null) ttft = performance.now() - t0; n++; text += d; onDelta(think ? `<think>${think}</think>${text}` : text); }
          if (v.usage?.completion_tokens) n = v.usage.completion_tokens;
        } catch {}
      }
    }
    const total = performance.now() - t0; return { text: think ? `<think>${think}</think>${text}` : text, ttft: ttft ?? total, total, tokens: n, tps: n * 1000 / Math.max(1, total - (ttft ?? 0)) };
  }
  let lastPrompt = "";
  $("#chat-form").onsubmit = async e => {
    e.preventDefault(); const q = $("#chat-input").value.trim(); if (!q) return;
    if ((S.run || {}).status !== "ready") return toast("start a model first (Overview → Run)");
    $("#chat-input").value = ""; lastPrompt = q; addMsg("user", q); history.push({ role: "user", content: q });
    const m = addMsg("assistant", "…"); $("#chat-send").disabled = true;
    try {
      const res = await streamChat("/v1", null, S.run.model, history, t => { m.innerHTML = renderThink(t); $("#chat").scrollTop = 1e9; });
      history.push({ role: "assistant", content: res.text });
      m.innerHTML = renderThink(res.text) + `<div class="meta">${fmt(res.ttft, 0)} ms TTFT · ${fmt(res.tps, 1)} tok/s · ${res.tokens} tok · ${fmt(res.total / 1000, 1)} s · ${esc(S.run.model)} on ${esc(devName(S.run.host_id))}${S.plan ? " · " + S.plan.mode : ""}</div>`;
      LAST_CHAT = res;
      $("#s-ttft").innerHTML = fmt(res.ttft, 0) + "<small>ms</small>"; $("#s-tps").innerHTML = fmt(res.tps, 1) + "<small>tok/s</small>"; $("#s-tok").textContent = res.tokens; $("#s-total").innerHTML = fmt(res.total / 1000, 1) + "<small>s</small>";
    } catch (err) { m.textContent = "✗ " + err.message; }
    $("#chat-send").disabled = false;
  };
  const renderThink = t => { const m = t.match(/^<think>([\s\S]*?)(<\/think>|$)([\s\S]*)$/); return m ? `<div class="think">${esc(m[1].trim())}</div>${esc(m[3].trim())}` : esc(t); };
  $("#ref-send").onclick = async () => {
    const url = $("#ref-url").value.trim(); const key = $("#ref-key").value.trim(); const model = $("#ref-model").value.trim();
    if (!url || !model || !lastPrompt) return toast("set endpoint + model and send a prompt locally first");
    try { $("#r-ttft").textContent = "…"; const res = await streamChat(url, key, model, [{ role: "user", content: lastPrompt }], () => {}); $("#r-ttft").innerHTML = fmt(res.ttft, 0) + "<small>ms</small>"; $("#r-tps").innerHTML = fmt(res.tps, 1) + "<small>tok/s</small>"; toast(`cloud: ${fmt(res.tps, 1)} tok/s`); } catch (e) { toast("cloud: " + e.message); $("#r-ttft").textContent = "✗"; }
  };

  /* ---- runs / analytics ---- */
  function renderRuns() {
    if (view !== "runs") return;
    const rows = RUNS.slice().reverse();
    const by = {};
    for (const r of RUNS) { if (!r.tokens_out) continue; const k = `${r.model} · ${r.mode} · ${r.devices} dev · host ${devName(r.host)}`; (by[k] ||= []).push(r.tps); }
    const agg = Object.entries(by).map(([k, v]) => [k, v.reduce((a, b) => a + b, 0) / v.length, v.length]).sort((a, b) => b[1] - a[1]);
    const max = Math.max(1, ...agg.map(a => a[1]));
    $("#runs-bars").innerHTML = agg.map(([k, v, n]) => `<div class="bar"><span title="${esc(k)}">${esc(k)} <span class="muted xs">×${n}</span></span><div class="track"><i style="width:${100 * v / max}%"></i></div><span class="num">${fmt(v, 1)} tok/s</span></div>`).join("") || '<div class="muted sm">no requests yet — use Chat</div>';
    const ok = RUNS.filter(r => r.ok && r.tokens_out > 0);
    const avg = (f) => ok.length ? ok.reduce((a, r) => a + f(r), 0) / ok.length : NaN;
    $("#runs-stats").innerHTML = [["Requests", RUNS.length, ""], ["Avg decode", fmt(avg(r => r.tps), 1), "tok/s"], ["Avg TTFT", fmt(avg(r => r.ttft_ms), 0), "ms"]].map(([k, v, u]) => `<div class="stat"><div class="k">${k}</div><div class="v">${v}<small>${u}</small></div></div>`).join("");
    $("#runs-table").style.setProperty("--cols", "1.2fr 2fr 1fr 1fr .7fr .7fr .7fr .7fr");
    $("#runs-table").innerHTML = `<div class="tr th"><span>time</span><span>model</span><span>mode</span><span>host</span><span class="num">TTFT</span><span class="num">tok</span><span class="num">tok/s</span><span class="num">total</span></div>` +
      rows.slice(0, 200).map(r => `<div class="tr"><span class="mono xs">${new Date(r.ts_ms).toLocaleTimeString()}</span><span>${esc(r.model)}</span><span>${esc(r.mode)}</span><span>${esc(devName(r.host))}</span><span class="num">${r.ttft_ms} ms</span><span class="num">${r.tokens_out}</span><span class="num">${fmt(r.tps, 1)}</span><span class="num">${(r.total_ms / 1000).toFixed(1)} s</span></div>`).join("");
  }

  /* ---- boot ---- */
  (async () => { try { CAT = await api("/api/catalog"); } catch {} await poll(); setInterval(poll, 2000); })();
})();
