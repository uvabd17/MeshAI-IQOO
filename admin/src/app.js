/* MeshAI admin — a guided four-step flow (1 Devices, 2 Models, 3 Prepare, 4 Use), plain language.
   Fit, feasibility and timing verdicts come from the server (POST /api/feasibility, POST /api/calculate,
   /api/state.advice, /api/state.last_run), never guessed client-side. New endpoints this panel calls
   (feasibility, usb/pair care, GET /api/usb/{serial}/care) may not exist on an older meshd yet — every
   call to them is wrapped so a 404 / network error degrades to a one-line note instead of breaking the
   page (docs/MESHAI.md §20). Vanilla JS, no build step. */
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
  const fmtSecs = s => (s == null || isNaN(s)) ? "—" : (s < 60 ? `${s.toFixed(0)} s` : `${(s / 60).toFixed(1)} min`);
  const esc = s => String(s ?? "").replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
  // Server Src (meshcore::cost::Src, serialised snake_case) → the four honesty tags the panel
  // shows: m measured, d derived, e estimate, ? unknown (docs/MESHAI.md §17.4).
  const srcTag = src => src === "measured" ? "m" : src === "derived" ? "d" : src === "estimate" ? "e" : "u";
  const TAG_LABEL = { m: "m", d: "d", e: "e", u: "?" };
  const dtag = tag => tag ? `<span class="dtag ${tag}" title="${{ m: "measured", d: "derived", e: "estimate", u: "unknown" }[tag] || ""}">${TAG_LABEL[tag] || tag}</span>` : "";
  const toast = m => { const t = $("#toast"); t.textContent = m; t.hidden = false; clearTimeout(t._t); t._t = setTimeout(() => t.hidden = true, 3200); };
  const median = arr => { const a = arr.filter(x => x > 0).slice().sort((x, y) => x - y); if (!a.length) return null; const m = Math.floor(a.length / 2); return a.length % 2 ? a[m] : (a[m - 1] + a[m]) / 2; };

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
    lock: '<rect x="5" y="10" width="14" height="10" rx="2"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/>',
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

  /* ---- theme ---- */
  const root = document.documentElement;
  try { root.dataset.theme = localStorage.getItem("meshai-theme") || "light"; } catch {}
  $("#theme").onclick = () => { root.dataset.theme = root.dataset.theme === "dark" ? "light" : "dark"; try { localStorage.setItem("meshai-theme", root.dataset.theme); } catch {} };

  /* ---- state ---- */
  let S = { devices: [], models: [], run: {}, plan: null, downloads: [] };
  let CAT = [], RUNS = [], OFFER = null, USB = { adb: false, devices: [] }, usbBusy = false;
  let LAST_CHAT = null, CHATTING = false, SELECTED = null;
  let LAPTOP_ONLY = false; try { LAPTOP_ONLY = localStorage.getItem("meshai-laptop-only") === "1"; } catch {}
  try { const sel = localStorage.getItem("meshai-selected-model"); if (sel) SELECTED = sel; } catch {}
  let FIT = {}; // per-model fit badge cache (fallback path), keyed by model file, refreshed from POST /api/plan
  let CALC_PLAN = null, CALC_RESP = null, CALC_ARGS = null, CALC_KEY_DONE = "", CALC_BUSY = false, PREDICTED = null;
  // /api/calculate is the source of truth for step 3 (§17.4). The client-side estimate below
  // (timeEstimate) only runs when that endpoint itself is unreachable (non-2xx / network error) —
  // CALC_FALLBACK marks that state so the UI can say so, distinct from a normal "cannot run"
  // verdict, which the endpoint itself returns with ok:false and a 200.
  let CALC_FALLBACK = false, CALC_CONN_ERROR = null, CALC_PLAN_ERROR = null;
  // Feasibility (T096, POST /api/feasibility): easy / hard-but-doable / not-possible categories for
  // every catalog model, with the concrete change that would make each one possible. Falls back to
  // the old per-model /api/plan fit check when the endpoint is not there yet (older meshd).
  let FEAS = null, FEAS_SIG = "", FEAS_UNAVAILABLE = false, FEAS_BUSY = false;
  // Downloads: remembers the very first byte count meshd reported for a file that started already
  // partway through, so the Prepare step can say "continuing from X MB" (a resumed `.part`).
  let DL_INITIAL = {};
  // Recovery banner (requirement 6): tracks each phone's last-seen online state to notice an
  // online→false flip, and whether we already showed the "start again" nudge for a given last_run.
  let PREV_PHONE_ONLINE = {};
  let AUTO_ADVANCED_FOR = null; // r.started_ms of the run we already auto-advanced to Use for
  const devById = id => S.devices.find(d => d.id === id);
  const devName = id => (devById(id) || { name: id }).name;
  const phones = () => S.devices.filter(d => !d.is_local && d.online);
  const online = () => S.devices.filter(d => d.online);
  const pooled = () => online().reduce((a, d) => a + (d.usable_bytes || 0), 0);
  const catOf = file => CAT.find(c => c.file === file);
  const isVision = file => !!(catOf(file) && catOf(file).mmproj);
  const isProjector = file => /^mmproj-/i.test(file) || (catOf(file) && catOf(file).role === "projector");

  /* ---- gating (requirement 1): four steps, locked until their own condition holds. Persisted in
     localStorage, but showStep() below never lands on a step whose lock is not satisfied — it
     clamps to the furthest step that is. */
  const STEP_ORDER = ["devices", "models", "prepare", "use"];
  const pairedOrLaptopOnly = () => phones().length > 0 || LAPTOP_ONLY;
  function lockReasonFor(step) {
    if (step === "models" || step === "prepare" || step === "use") {
      if (!pairedOrLaptopOnly()) return "pair a phone first, or choose “this laptop only”";
    }
    if (step === "prepare" || step === "use") {
      if (!SELECTED) return "pick a model first";
    }
    if (step === "use") {
      if (!((S.run || {}).status === "ready")) return "finish Prepare first";
    }
    return null;
  }
  function furthestUnlocked() {
    let f = "devices";
    for (const s of STEP_ORDER) { if (lockReasonFor(s)) break; f = s; }
    return f;
  }

  /* ---- four-step nav ---- */
  let STEP = "devices";
  const showStep = s => {
    const reason = lockReasonFor(s);
    if (reason) { toast(reason); s = furthestUnlocked(); }
    STEP = s;
    $$("#steps button").forEach(b => b.classList.toggle("active", b.dataset.step === s));
    $$(".page").forEach(sec => sec.hidden = sec.id !== "step-" + s);
    try { localStorage.setItem("meshai-step", s); } catch {}
    render();
  };
  $$("#steps button").forEach(b => b.onclick = () => showStep(b.dataset.step));
  $("#to-models").onclick = () => showStep("models");
  $("#to-devices2").onclick = () => showStep("devices");
  $("#to-prepare").onclick = () => showStep("prepare");
  $("#to-models2").onclick = () => showStep("models");
  $("#laptop-only-btn").onclick = () => {
    LAPTOP_ONLY = true;
    try { localStorage.setItem("meshai-laptop-only", "1"); } catch {}
    toast("continuing with this laptop only");
    render();
  };
  function renderStepNav() {
    STEP_ORDER.forEach(s => {
      const btn = $(`#steps button[data-step="${s}"]`);
      if (!btn) return;
      const reason = lockReasonFor(s);
      btn.classList.toggle("locked", !!reason);
      btn.title = reason || "";
      const li = $(".lock-i", btn); if (li) li.hidden = !reason;
    });
    const nextBtns = [["#to-models", "models"], ["#to-prepare", "prepare"]];
    nextBtns.forEach(([sel, step]) => { const b = $(sel); if (!b) return; const reason = lockReasonFor(step); b.disabled = !!reason; b.title = reason || ""; });
  }

  /* ---- developer Details drawer (replaces the old Advanced toggle) ---- */
  let DETAILS_OPEN = false;
  const openDetails = () => { DETAILS_OPEN = true; $("#details").hidden = false; $("#details-backdrop").hidden = false; render(); };
  const closeDetails = () => { DETAILS_OPEN = false; $("#details").hidden = true; $("#details-backdrop").hidden = true; };
  $("#details-btn").onclick = openDetails;
  $("#details-close").onclick = closeDetails;
  $("#details-backdrop").onclick = closeDetails;

  async function poll() {
    try {
      S = await api("/api/state");
      (S.downloads || []).forEach(d => { if (d.done) delete DL_INITIAL[d.file]; else if (!(d.file in DL_INITIAL)) DL_INITIAL[d.file] = d.bytes; });
      if (DETAILS_OPEN || STEP === "devices") { try { RUNS = await api("/api/runs"); } catch {} }
      render();
    } catch (e) { $("#now-title").textContent = "meshd is not reachable"; $("#status-text").textContent = "offline"; }
  }
  async function pollUsb() { try { USB = await api("/api/usb"); } catch { USB = { adb: false, devices: [] }; } if (STEP === "devices") renderUsb(); }

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
      case "error": return [`Stopped: ${r.error || "error"}`, "see the banner below for what to do", null];
      default: return phones().length ? ["Devices connected — pick a model and calculate", `${online().length} devices online · ${gb(pooled())} for models`, null] : ["Connect your phone to begin", "USB cable or QR, or continue with this laptop only — on step 1", null];
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
    $("#mesh-id").textContent = S.mesh_id || "—";
    // Auto-advance to Use once the run this Prepare page started is ready (requirement 4).
    if (STEP === "prepare" && r.status === "ready" && r.started_ms !== AUTO_ADVANCED_FOR) { AUTO_ADVANCED_FOR = r.started_ms; showStep("use"); }
  }

  /* ---- recovery banner (requirement 6): run.status == "error", a paired phone going offline, and
     meshd restarting with a remembered last_run. Raw error strings from desktop/meshd/src (grepped:
     "went offline", "not reachable", "Failed to create server socket", "can hold only") are mapped to
     plain sentences; anything unrecognised is shown as-is so nothing is silently swallowed. */
  function mapRunError(msg) {
    if (!msg) return null;
    const m = msg.toLowerCase();
    if (m.includes("went offline")) {
      const dm = msg.match(/device (\S+) went offline/);
      const name = dm ? devName(dm[1]) : "the phone";
      return `The phone went offline during the run — the run was stopped. Open the MeshAI app on ${esc(name)}, then press Start again; layers already on the phone are reused.`;
    }
    if (m.includes("failed to create server socket") || (m.includes("port") && (m.includes("reserv") || m.includes("in use")))) {
      return "Port was reserved by the phone — press Start again (it tries other ports).";
    }
    if (m.includes("not reachable")) {
      return "A device could not be reached over the link — check the cable or Wi-Fi, then press Start again.";
    }
    if (m.includes("can hold only") || m.includes("pool only") || m.includes("close apps")) {
      const dm = msg.match(/^host (\S+) can hold only/) || msg.match(/^([\w .-]+) can hold only/);
      const name = dm ? devName(dm[1]) || dm[1] : "a device";
      return `Out of memory on ${esc(name)} — close apps or use 2k context, then press Start again.`;
    }
    return null;
  }
  function renderRecovery() {
    const el = $("#recovery-banner"); if (!el) return;
    const r = S.run || {};
    let html = null, level = "warn";
    if (r.status === "error" && r.error) {
      const mapped = mapRunError(r.error);
      html = mapped || `${esc(r.error)} <span class="muted xs">(unrecognised error — showing it as reported)</span>`;
      level = "fail";
    } else {
      const dropped = S.devices.find(d => !d.is_local && PREV_PHONE_ONLINE[d.id] === true && d.online === false);
      if (dropped) {
        html = `${esc(dropped.name)} went offline. Open the MeshAI app on the phone to reconnect — it rejoins on its own with the stored pairing.`;
        level = "warn";
      } else if (S.last_run && r.status === "idle") {
        const lr = S.last_run;
        html = `meshd restarted: your last setup was <b>${esc((lr.model || "").replace(/\.gguf$/, ""))}</b>. <button class="btn sm" id="recovery-start-again">Start again</button>`;
        level = "info";
      }
    }
    S.devices.forEach(d => { if (!d.is_local) PREV_PHONE_ONLINE[d.id] = d.online; });
    if (!html) { el.hidden = true; el.innerHTML = ""; return; }
    el.hidden = false;
    el.className = "recovery-banner lvl-" + level;
    el.innerHTML = html;
    drawIcons(el);
    const again = $("#recovery-start-again", el);
    if (again) again.onclick = () => {
      const lr = S.last_run; if (!lr) return;
      SELECTED = lr.model; try { localStorage.setItem("meshai-selected-model", SELECTED); } catch {}
      if (lr.n_ctx && $("#qs-ctx")) { $("#qs-ctx").value = String(lr.n_ctx); }
      showStep("prepare");
      toast("setup restored from before the restart — check Prepare, then press Start");
    };
  }

  /* ---- step 1: devices ---- */
  function renderUsb() {
    const el = $("#usb"); if (!el) return;
    const paired = phones().length > 0;
    $("#usb-sub").textContent = !USB.adb ? "adb not found on this laptop" : USB.devices.length ? `${USB.devices.length} phone${USB.devices.length > 1 ? "s" : ""} on USB` : "no phone with USB debugging found";
    el.innerHTML = USB.devices.map(d => `<div class="u">${DEV_ICON.phone}<div class="grow"><div class="u-name">${esc(d.model)}</div><div class="u-serial">${esc(d.serial)}${paired ? " · connected" : ""}</div></div><button class="btn primary" data-usb="${esc(d.serial)}" ${usbBusy ? "disabled" : ""}>${svg("phone")} ${paired ? "Re-pair" : "Pair over USB"}</button></div>`).join("");
    $$("[data-usb]", el).forEach(b => b.onclick = async () => {
      usbBusy = true; renderUsb();
      try {
        const r = await api("/api/usb/pair", { method: "POST", body: JSON.stringify({ serial: b.dataset.usb }) });
        toast("Now tap Join on the phone");
        renderCare(r.care); // T097: after a successful pair, show what USB pairing turned on (or a note if meshd does not report it yet)
      } catch (e) { toast(e.message); }
      usbBusy = false; renderUsb();
    });
  }
  function careSentence(care) {
    if (!care || typeof care !== "object") return null;
    const entries = Object.entries(care);
    if (!entries.length) return null;
    return entries.map(([k, v]) => `${esc(k.replace(/_/g, " "))}: ${v === true ? "on" : v === false ? "off" : esc(String(v))}`).join(" · ");
  }
  function renderCare(care) {
    const el = $("#care-note"); if (!el) return;
    const sentence = careSentence(care);
    if (!sentence) { el.hidden = true; el.innerHTML = ""; return; }
    el.hidden = false;
    el.innerHTML = `<div class="dv-h">phone controls turned on over USB</div><div class="muted sm">${sentence}</div>`;
  }
  function coreBars(d, big) {
    const tel = d.telemetry || {}; const loads = tel.core_loads || []; const caps = ((d.profile || {}).cores || []).map(c => c.capacity);
    if (!loads.length) return "";
    return `<div class="cores ${big ? "big" : ""}" title="each bar = one CPU core · taller = running faster right now">${loads.map((l, i) => `<i class="${(caps[i] || 1024) >= 1000 ? "prime" : (caps[i] || 1024) >= 500 ? "big" : "small"}" style="height:${Math.max(8, Math.round(l * 100))}%"></i>`).join("")}</div>`;
  }
  function specLine(d) {
    const p = d.profile || {}; const parts = [];
    if (p.soc) parts.push(p.soc.split(" · ")[0]);
    const cores = (p.cores || []).length; if (cores) parts.push(`${cores} cores`);
    if (p.total_bytes) parts.push(gb(p.total_bytes) + " RAM");
    if (p.os) parts.push(p.os.split(" (")[0]);
    return parts.join(" · ") || (d.online ? "waiting for specs…" : "offline");
  }
  function roomBlock(d) {
    const tel = d.telemetry || {}; const total = (d.profile || {}).total_bytes || 0; const free = tel.avail_bytes || 0; const usable = d.usable_bytes || 0;
    const bar = total ? `<div class="membar"><i class="used" style="width:${(100 * (total - free) / total).toFixed(0)}%"></i><i class="keep" style="width:${(100 * Math.max(0, free - usable) / total).toFixed(0)}%"></i><i class="free" style="width:${(100 * usable / total).toFixed(0)}%"></i></div>` : "";
    return `<div class="dv-h">room for models: ${gb(usable)} free</div>${bar}${coreBars(d, false)}`;
  }
  // Role chip (requirement 2): computed from the current state, not just from an active plan —
  // HOST / HELPER / NOT USABLE with the reason, plus an engine chip. Only llama.cpp exists today
  // (docs §20), so the engine chip is a single fixed tag until /api/state lists engines.
  function roleChip(d) {
    if (!d.online) return { label: "NOT USABLE", cls: "no", reason: "offline" };
    if (d.is_local) return { label: (S.plan && S.plan.host_id === d.id) ? "HOST" : "HOST-CAPABLE", cls: "host", reason: "" };
    if (kindOf(d) === "sim") return { label: "HELPER (simulated)", cls: "worker", reason: "" };
    if (S.plan) {
      const p = S.plan.placements.find(x => x.device_id === d.id);
      if (p && p.role === "Host") return { label: "HOST", cls: "host", reason: "" };
      if (p && p.role === "Worker") return { label: "HELPER", cls: "worker", reason: "" };
    }
    const tel = d.telemetry || {}; const rtt = tel.rtt_ms_p95 || 0; const room = d.usable_bytes || 0;
    const battery = tel.battery_pct;
    if (rtt > 60) return { label: "NOT USABLE", cls: "no", reason: "slow link" };
    if (room < 1e9) return { label: "NOT USABLE", cls: "no", reason: "no room" };
    if (battery != null && battery < 15 && !tel.charging) return { label: "NOT USABLE", cls: "no", reason: "battery" };
    return { label: "HELPER", cls: "worker", reason: "" };
  }
  function capLine(d) {
    if (!d.online) return "offline";
    if (d.is_local) return "can host, can help";
    if (kindOf(d) === "sim") return "simulated helper (testing only)";
    const tel = d.telemetry || {}; const rtt = tel.rtt_ms_p95 || 0;
    const link = d.addr === "127.0.0.1" ? "USB cable" : d.addr ? "network" : "not linked";
    const slow = rtt > 60;
    return `${slow ? "slow link · try USB tethering" : "can help"} · link ${link}${rtt ? ` · ${fmt(rtt, 0)} ms` : ""}`;
  }
  function deviceCard(d) {
    const p = S.plan ? S.plan.placements.find(x => x.device_id === d.id) : null; const used = p && p.role !== "Rejected";
    const role = roleChip(d);
    return `<div class="dv ${used ? (p.role === "Host" ? "host" : "worker") : ""} ${d.online ? "" : "offline"}">
      <div class="dv-top">${DEV_ICON[kindOf(d)]}<div class="grow"><div class="dv-name">${esc(d.name)}</div><div class="dv-role">${d.is_local ? "this laptop" : "phone"} · ${d.online ? "online" : "offline"}</div></div>
        <div class="chip-col"><span class="rolechip ${role.cls}">${esc(role.label)}</span>${role.reason ? `<span class="muted xs">${esc(role.reason)}</span>` : ""}<span class="chip engine-chip">llama.cpp</span></div></div>
      <div class="dv-sec"><div class="dv-h">${esc(specLine(d))}</div><div class="muted sm">${esc(capLine(d))}</div></div>
      <div class="dv-sec">${roomBlock(d)}</div>
      ${used ? `<div class="dv-sec"><div class="dv-h">its share of the model</div><div class="mn-layers">layers ${p.layer_start}–${Math.max(p.layer_start, p.layer_end - 1)} · ${p.layer_end - p.layer_start} of ${nLayers()} · ${gb(p.bytes)}</div>${layerBar(p)}</div>` : ""}
      <div class="dv-sec row"><label class="sm muted">usable cap (GB) <input class="sm" style="width:80px" type="number" step="0.5" value="${d.usable_override_bytes ? (d.usable_override_bytes / 1e9).toFixed(1) : ""}" placeholder="auto" data-limit="${esc(d.id)}"></label>${d.is_local ? "" : `<button class="btn sm ghost" data-forget="${esc(d.id)}">Forget</button>`}</div>
    </div>`;
  }
  function renderDevices() {
    const el = $("#devices2");
    const list = S.devices.slice().sort((a, b) => (a.is_local ? -1 : 1) - (b.is_local ? -1 : 1) || (b.online - a.online));
    el.innerHTML = list.map(deviceCard).join("") + (phones().length ? "" : `<div class="dv placeholder">${DEV_ICON.phone}<div class="dv-name">Your phone</div><div class="muted sm">appears here once it joins — specs, room for models and its share of the plan</div></div>`);
    const p = phones()[0]; const tel = p && p.telemetry || {};
    $("#link-sub").textContent = p ? `linked ${p.addr === "127.0.0.1" ? "over the USB cable" : "over " + p.addr} · round trip ${fmt(tel.rtt_ms_p50, 1)} ms · ${gb(pooled())} for models across ${online().length} devices` : "";
    $("#connect-card").classList.toggle("done", phones().length > 0);
    $$("[data-limit]", el).forEach(i => i.onchange = async () => { await api(`/api/devices/${i.dataset.limit}/limit`, { method: "POST", body: JSON.stringify({ usable_gb: i.value ? +i.value : null }) }); toast("cap updated"); poll(); });
    $$("[data-forget]", el).forEach(b => b.onclick = async () => { await api(`/api/devices/${b.dataset.forget}`, { method: "DELETE" }); poll(); });
    if (OFFER) { $("#qr").innerHTML = OFFER.svg; $("#qr-text").innerHTML = `The phone will try: ${(OFFER.links || []).map(l => `<span class="tag">${esc(l.kind)}</span> <span class="mono">${esc(l.ip)}</span>`).join(" → ")}. One-time code, valid 10 minutes.`; }
  }
  // Advice (T096, /api/state.advice): a short checklist under the cards, level-coloured. `S.advice`
  // is a plain field of /api/state, so there is no 404 to catch — an older meshd simply omits it,
  // and that is treated the same way as a missing endpoint: hide the feature, one line explaining why.
  function renderAdvice() {
    const el = $("#advice-list"); if (!el) return;
    if (!Array.isArray(S.advice)) { el.innerHTML = `<div class="muted sm">per-device advice is not available from this meshd yet.</div>`; return; }
    if (!S.advice.length) { el.innerHTML = ""; return; }
    el.innerHTML = S.advice.map(a => `<div class="advice-item lvl-${esc(a.level || "info")}">${esc(a.text)}${a.action ? ` — <b>${esc(typeof a.action === "string" ? a.action : (a.action.label || JSON.stringify(a.action)))}</b>` : ""}</div>`).join("");
  }
  function renderMeshSummary() {
    const show = pairedOrLaptopOnly();
    $("#mesh-summary").hidden = !show;
    if (!show) return;
    const p = phones()[0];
    const best = RUNS.filter(r => r.ok && r.tokens_out > 0).reduce((m, r) => (r.tps > (m ? m.tps : -1) ? r : m), null);
    const rows = [];
    rows.push(["total room for models", `${gb(pooled())} across ${online().length} device${online().length === 1 ? "" : "s"}`]);
    rows.push(["best measured speed", best ? `${fmt(best.tps, 1)} tok/s${dtag("m")} (${esc(best.model)})` : "no runs measured yet"]);
    rows.push(["link", p ? `${p.addr === "127.0.0.1" ? "USB cable" : p.addr ? "network" : "not linked"} · RTT ${fmt((p.telemetry || {}).rtt_ms_p50, 1)} ms` : "this laptop only"]);
    $("#mesh-summary-body").innerHTML = rows.map(([k, v]) => `<div class="ms-row"><b>${esc(k)}</b><span>${v}</span></div>`).join("");
  }

  /* ---- shared: layer bar helper (used by devices cards + prepare step) ---- */
  const nLayers = () => { const plan = S.plan || CALC_PLAN; const m = plan && S.models.find(x => x.file === plan.model); return m ? m.info.n_layer : (plan ? Math.max(...plan.placements.map(p => p.layer_end)) : 0); };
  const layerBar = p => { const nl = nLayers() || 1; return `<div class="mn-bar"><i style="left:${(p.layer_start / nl * 100).toFixed(1)}%;width:${((p.layer_end - p.layer_start) / nl * 100).toFixed(1)}%"></i></div>`; };

  /* ---- step 2: models ---- */
  const PROVIDER = { qwen: ["Q", "Qwen (Alibaba)"], openai: ["O", "OpenAI"], google: ["G", "Google"], meta: ["M", "Meta"], mistral: ["Mi", "Mistral"], huggingface: ["HF", "Hugging Face"] };
  function providerOf(m) { const c = catOf(m.file); if (c && PROVIDER[c.provider]) return PROVIDER[c.provider]; const n = (m.info.name || m.file).toLowerCase(); if (n.includes("qwen")) return PROVIDER.qwen; if (n.includes("gpt") || n.includes("oss")) return PROVIDER.openai; if (n.includes("gemma")) return PROVIDER.google; if (n.includes("llama")) return PROVIDER.meta; if (n.includes("mistral")) return PROVIDER.mistral; return ["M", "model"]; }
  const paramsOf = m => { const s = (m.info.name || m.file); const a = s.match(/(\d+(?:\.\d+)?)B-A(\d+(?:\.\d+)?)B/i); if (a) return `${a[1]}B total · ${a[2]}B active`; const b = s.match(/(\d+(?:\.\d+)?)\s?B\b/i); return b ? `${b[1]}B parameters` : "—"; };
  const ktok = n => n >= 1000 ? `${Math.round(n / 1000)}k tokens` : `${n} tokens`;

  const selectModel = file => { SELECTED = file; try { localStorage.setItem("meshai-selected-model", SELECTED); } catch {} renderModels(); renderStepNav(); };

  /* Fit badges (fallback path only) come from the server (POST /api/plan) — no client-side memory
     guess. Cached per model file + context + online-device signature. */
  const fitSig = () => `${$("#qs-ctx").value}|${online().map(d => d.id).sort().join(",")}`;
  function fitOf(m) {
    const sig = fitSig(); const c = FIT[m.file];
    if (c && c.sig === sig) return c;
    computeFit(m, sig);
    return { label: "checking…", cls: "" };
  }
  async function computeFit(m, sig) {
    FIT[m.file] = { sig, label: "checking…", cls: "" };
    try {
      const r = await api("/api/plan", { method: "POST", body: JSON.stringify({ model: m.file, n_ctx: +$("#qs-ctx").value, host: null }) });
      const p = r.plan; const used = p.placements.filter(x => x.role !== "Rejected").length;
      FIT[m.file] = { sig, label: p.mode === "Single" ? `fits ${devName(p.host_id)} alone` : `needs ${used} devices`, cls: p.mode === "Single" ? "ok" : "split" };
    } catch (e) {
      FIT[m.file] = { sig, label: "cannot run now", cls: "no" };
    }
    if (STEP === "models") renderModels();
  }
  function modelCard(m) {
    const on = m.file === SELECTED; const vis = isVision(m.file); const [mark, prov] = providerOf(m);
    const arch = (m.info.arch || "").toLowerCase(); const tools = /qwen3|gpt-oss|llama|mistral|gemma/.test(arch) || /qwen3|gpt-oss/i.test(m.info.name || ""); const thinks = /qwen3|gpt-oss/i.test(arch + (m.info.name || ""));
    const c = catOf(m.file); const fit = fitOf(m);
    return `<button class="mc ${on ? "on" : ""} ${fit.cls}" data-model="${esc(m.file)}" title="${esc(c ? c.note : m.file)}">
      <div class="mc-head"><div class="mc-mark">${esc(mark)}</div><div><div class="mc-name">${esc(m.info.name || m.file)}</div><div class="mc-meta">${esc(prov)}${c && c.role ? " · " + esc(c.role) : ""}</div></div></div>
      <div class="mc-spec"><b>size</b><span>${paramsOf(m)} · ${esc(m.info.quant_label)} · ${gb(m.info.file_bytes)} file</span><b>shape</b><span>${m.info.n_layer} layers${m.info.n_expert > 1 ? ` · ${m.info.n_expert} experts, ${m.info.n_expert_used} used` : ""}</span><b>memory</b><span>remembers up to ${ktok(m.info.n_ctx_train)} (${(m.info.kv_bytes_per_token / 1024).toFixed(0)} KB per token)</span></div>
      <div class="mc-io"><i class="yes">text in</i><i class="${vis ? "img" : "no"}">${vis ? "images in" : "no images"}</i><i class="yes">text out</i><i class="${tools ? "yes" : "no"}">${tools ? "tool calls" : "no tools"}</i><i class="${thinks ? "yes" : "no"}">${thinks ? "can think" : "direct"}</i></div>
      <div class="mc-fit">${esc(fit.label)}</div>${(S.run || {}).model === m.file && (S.run || {}).status === "ready" ? `<div class="mc-live">running</div>` : ""}
    </button>`;
  }
  function renderFallbackGroups() {
    const runnable = S.models.filter(m => !isProjector(m.file));
    const buckets = { easy: [], hard: [], impossible: [] };
    runnable.forEach(m => { const cls = fitOf(m).cls; buckets[cls === "ok" ? "easy" : cls === "no" ? "impossible" : "hard"].push(m); });
    $("#cards-easy").innerHTML = buckets.easy.map(modelCard).join("") || `<div class="muted sm">none fit alone right now.</div>`;
    $("#cards-hard").innerHTML = buckets.hard.map(modelCard).join("") || `<div class="muted sm">none.</div>`;
    $("#cards-impossible").innerHTML = buckets.impossible.map(modelCard).join("") || `<div class="muted sm">none.</div>`;
    $$("#cards-easy [data-model], #cards-hard [data-model], #cards-impossible [data-model]").forEach(b => b.onclick = () => selectModel(b.dataset.model));
  }

  // Needs (T096 `needs[]`): plain sentence + an action button where one makes sense. Field shapes
  // (`detail`, `device_id`, `bytes`) are read defensively since the endpoint is new and still being
  // built by another engineer — a missing/odd field degrades to a safe generic phrase, never a crash.
  // `detail` is already a full, human sentence from meshd (measured: "free about 0.18 GB more on
  // <device> to run without splitting…", "pair one more device with at least 1.78 GB free…",
  // "download gpt-oss-20b-mxfp4.gguf (12.10 GB)") — shown verbatim. NEED_TEXT is only the fallback
  // for a missing `detail`.
  const NEED_TEXT = {
    context: n => "Use a shorter context",
    free_memory: n => `Close apps on ${n.device_id ? devName(n.device_id) : "the device"}`,
    device: n => "Add another device",
    quant: n => "Use a smaller quant",
    download: n => "Download the model first",
  };
  function needSentence(n) {
    if (n.detail) return n.detail.charAt(0).toUpperCase() + n.detail.slice(1);
    const mk = NEED_TEXT[n.what]; return mk ? mk(n) : (n.what || "needs a change");
  }
  function parseCtxDetail(detail) {
    const opts = [2048, 4096, 8192, 16384];
    let n = parseInt(String(detail || "").replace(/[^0-9]/g, ""), 10);
    if (!n) return 2048;
    if (/k/i.test(String(detail)) && n < 100) n *= 1024;
    const fit = opts.find(o => o >= n);
    return fit || opts.reduce((a, b) => Math.abs(b - n) < Math.abs(a - n) ? b : a, opts[0]);
  }
  function needRow(n, item) {
    let action = "";
    if (n.what === "context") action = `<button class="btn sm ghost" data-need-ctx="${esc(n.detail || "")}">Use a shorter context</button>`;
    else if (n.what === "free_memory") action = `<button class="btn sm ghost" disabled title="close it yourself on the device">Close apps${n.device_id ? ` on ${esc(devName(n.device_id))}` : ""}</button>`;
    else if (n.what === "device") action = `<button class="btn sm ghost" data-need-adddevice="1">Add a device</button>`;
    else if (n.what === "download") action = `<button class="btn sm" data-need-download="${esc(item.id || "")}">Download</button>`;
    return `<div class="need-row"><span>${esc(needSentence(n))}</span>${action}</div>`;
  }
  function feasCard(item) {
    const chosen = item.file === SELECTED;
    const cat = catOf(item.file); const onModel = S.models.find(m => m.file === item.file);
    const name = item.name || (cat && cat.name) || (onModel && onModel.info.name) || item.file;
    const sizeLine = onModel ? `${gb(onModel.info.file_bytes)} · ${onModel.info.n_layer} layers` : (cat ? `${gb(cat.bytes)}${cat.layers ? ` · ${cat.layers} layers` : ""}` : "");
    const clickable = item.category !== "impossible";
    const tone = item.category === "easy" ? "ok" : item.category === "hard" ? "split" : "no";
    const tps = item.est_tok_s && item.est_tok_s.v != null ? `<div class="mc-fit">${fmt(item.est_tok_s.v, 1)} tok/s${dtag(srcTag(item.est_tok_s.src))}</div>` : "";
    return `<div class="mc ${chosen ? "on" : ""} ${tone} ${clickable ? "" : "mc-disabled"}" ${clickable ? `data-model="${esc(item.file)}"` : ""} title="${esc(item.file)}">
      <div class="mc-head"><div class="mc-mark">${esc((name || "M")[0].toUpperCase())}</div><div><div class="mc-name">${esc(name)}</div><div class="mc-meta">${item.on_disk ? "on disk" : "not downloaded"}</div></div></div>
      <div class="muted sm">${esc(sizeLine)}</div>
      ${item.category === "impossible" ? `<div class="mc-fit">${esc(item.reason || "not possible with the current devices")}</div>` : ""}
      ${item.category === "hard" && (item.needs || []).length ? `<div class="fc-needs">${item.needs.map(n => needRow(n, item)).join("")}</div>` : ""}
      ${tps}
    </div>`;
  }
  function renderFeasGroups() {
    const groups = { easy: [], hard: [], impossible: [] };
    (FEAS || []).forEach(item => { (groups[item.category] || groups.hard).push(item); });
    $("#cards-easy").innerHTML = groups.easy.map(feasCard).join("") || `<div class="muted sm">none fit alone right now.</div>`;
    $("#cards-hard").innerHTML = groups.hard.map(feasCard).join("") || `<div class="muted sm">none.</div>`;
    $("#cards-impossible").innerHTML = groups.impossible.map(feasCard).join("") || `<div class="muted sm">none.</div>`;
    $$("#cards-easy [data-model], #cards-hard [data-model], #cards-impossible [data-model]").forEach(c => c.onclick = e => {
      if (e.target.closest("[data-need-ctx],[data-need-adddevice],[data-need-download]")) return;
      selectModel(c.dataset.model);
    });
    $$("[data-need-ctx]").forEach(b => b.onclick = e => { e.stopPropagation(); const v = parseCtxDetail(b.dataset.needCtx); $("#qs-ctx").value = String(v); $("#qs-ctx").dispatchEvent(new Event("change")); toast(`context set to ${v}`); });
    $$("[data-need-adddevice]").forEach(b => b.onclick = e => { e.stopPropagation(); showStep("devices"); });
    $$("[data-need-download]").forEach(b => b.onclick = async e => {
      e.stopPropagation(); const id = b.dataset.needDownload; if (!id) return toast("no catalog id for this model");
      try { await api("/api/models/download", { method: "POST", body: JSON.stringify({ id }) }); toast("download started"); poll(); } catch (err) { toast(err.message); }
    });
  }
  async function ensureFeasibility() {
    const sig = fitSig();
    if (FEAS_SIG === sig || FEAS_BUSY) return;
    FEAS_BUSY = true;
    try {
      const r = await api("/api/feasibility", { method: "POST", body: JSON.stringify({ n_ctx: +$("#qs-ctx").value }) });
      FEAS = r.models || []; FEAS_UNAVAILABLE = false; FEAS_SIG = sig;
    } catch (e) {
      FEAS = null; FEAS_UNAVAILABLE = true; FEAS_SIG = sig;
    }
    FEAS_BUSY = false;
    if (STEP === "models") renderModels();
  }
  function renderModels() {
    $("#models-dir").textContent = S.models_dir || "";
    ensureFeasibility();
    if (!FEAS_UNAVAILABLE && FEAS) {
      $("#feasibility-note").textContent = "";
      renderFeasGroups();
    } else {
      $("#feasibility-note").textContent = "Detailed feasibility categories (POST /api/feasibility) are not available from this meshd yet — showing a basic fit check instead.";
      renderFallbackGroups();
    }
  }
  $("#qs-ctx").onchange = () => { FEAS_SIG = ""; if (STEP === "models") renderModels(); if (STEP === "prepare") renderPrepare(); };

  /* ---- step 3: prepare + calculate ---- */
  function fixHint(msg) {
    const m = (msg || "").toLowerCase();
    if (m.includes("rtt")) return "move the phone closer, or connect it with a USB cable instead of Wi-Fi.";
    if (m.includes("battery")) return "plug the phone in to charge.";
    if (m.includes("thermal") || m.includes("throttl")) return "let the device cool down, then try again.";
    if (m.includes("unsupported device")) return "this device does not meet the minimum chip requirements (arm64, dotprod + i8mm, 8 GB+) and cannot join.";
    return "try a shorter context (2k), a smaller model or quant, closing apps on the phone to free memory, or adding another device.";
  }
  function plainReason(reason) { return (reason || "").replace(/^rejected:\s*/i, "").replace(/^not needed:\s*/i, "not needed — "); }

  /* Fallback only: used when POST /api/calculate itself is unreachable (network error / non-2xx),
     not for an ordinary "cannot run" verdict (the endpoint answers that with ok:false and a 200).
     Measured history for this exact model+mode from /api/runs when it exists (tag m); otherwise
     scaled from any measured laptop-alone run by file-size ratio (tag e), plus the measured
     USB-debugging-cable cost (0.72 s/token, D032 §17.3) when the plan would put a worker behind
     adb's loopback forward; otherwise unknown. */
  function timeEstimate(plan, qin, qout) {
    const rows = RUNS.filter(r => r.ok && r.tokens_out > 0 && r.model === plan.model && r.mode === plan.mode);
    let promptTps = null, tps = null, tag = "e", note = "";
    if (rows.length) {
      promptTps = median(rows.map(r => r.prompt_tps)); tps = median(rows.map(r => r.tps));
      tag = "m"; note = `measured last time (${rows.length} earlier run${rows.length > 1 ? "s" : ""} of this exact setup)`;
    } else {
      const laptopRows = RUNS.filter(r => r.ok && r.tokens_out > 0 && r.mode === "Single");
      if (laptopRows.length) {
        const ref = laptopRows[laptopRows.length - 1];
        const refM = S.models.find(m => m.file === ref.model); const curM = S.models.find(m => m.file === plan.model);
        const ratio = (refM && curM && refM.info.file_bytes && curM.info.file_bytes) ? refM.info.file_bytes / curM.info.file_bytes : 1;
        promptTps = (ref.prompt_tps || 0) * ratio; tps = (ref.tps || 0) * ratio;
        note = `estimate, scaled from a measured run of ${esc(refM ? (refM.info.name || ref.model) : ref.model)} on this laptop`;
        const cablePhone = plan.mode === "LayerSplit" && plan.placements.some(p => p.role === "Worker" && (devById(p.device_id) || {}).addr === "127.0.0.1");
        if (cablePhone && tps > 0) { tps = 1 / ((1 / tps) + 0.72); note = "estimate from the measured adb split (adds 0.72 s per token for the USB-debugging cable)"; }
      }
    }
    if (!tps || !promptTps || tps <= 0 || promptTps <= 0) return { unknown: true };
    const firstWordS = qin / promptTps;
    return { tag, note, promptTps, tps, firstWordS, answerS: firstWordS + qout / tps };
  }
  /* Both the server prediction (meshcore::cost::Prediction) and the client-side fallback above
     are rendered through this one shape so calc-time / calc-history / renderPredicted do not care
     which one produced it. lo/hi come straight from the server's Est band; the fallback has no
     band of its own, so it uses the same ± bands the server would (measured 15 %, estimate 50 %,
     §17.4) to stay honest about the fallback being rougher, not better, than the real thing. */
  function normalizePrediction(pred) {
    if (!pred || !pred.tok_s || pred.tok_s.src === "unknown") return { unknown: true };
    return {
      tag: srcTag(pred.tok_s.src),
      ttftMs: pred.ttft_ms.v, ttftLo: pred.ttft_ms.lo, ttftHi: pred.ttft_ms.hi,
      tokS: pred.tok_s.v, tokSLo: pred.tok_s.lo, tokSHi: pred.tok_s.hi,
      totalMs: pred.total_ms.v, totalLo: pred.total_ms.lo, totalHi: pred.total_ms.hi,
      bottleneck: pred.bottleneck, note: null,
    };
  }
  function normalizeFallback(est) {
    if (!est || est.unknown) return { unknown: true };
    const band = est.tag === "m" ? 0.15 : 0.50;
    const spread = (v) => ({ v, lo: v * (1 - band), hi: v * (1 + band) });
    const ttft = spread(est.firstWordS * 1000), tok = spread(est.tps), total = spread(est.answerS * 1000);
    return {
      tag: est.tag,
      ttftMs: ttft.v, ttftLo: ttft.lo, ttftHi: ttft.hi,
      tokS: tok.v, tokSLo: tok.lo, tokSHi: tok.hi,
      totalMs: total.v, totalLo: total.lo, totalHi: total.hi,
      bottleneck: null, note: est.note,
    };
  }
  function renderCalcStack(plan) {
    const used = plan.placements.filter(p => p.role !== "Rejected").slice().sort((a, b) => a.layer_start - b.layer_start);
    return `<div class="stackbar"><i class="pin" title="embeddings + output head, always on the host">in/out</i>${used.map(p => `<i class="${p.role === "Host" ? "host" : "worker"}" style="flex:${Math.max(1, p.layer_end - p.layer_start)}" title="${esc(p.name)} · layers ${p.layer_start}–${Math.max(p.layer_start, p.layer_end - 1)}">${p.layer_end - p.layer_start}</i>`).join("")}</div>
      <div class="legend"><span><b class="pin"></b>embeddings + output · always on the host</span>${used.map(p => `<span><b class="${p.role === "Host" ? "host" : "worker"}"></b>${esc(p.name)} · layers ${p.layer_start}–${Math.max(p.layer_start, p.layer_end - 1)}</span>`).join("")}</div>`;
  }
  function renderCalcTable(plan, prediction) {
    const stacksById = {}; ((prediction && prediction.stacks) || []).forEach(s => stacksById[s.device_id] = s);
    const rows = plan.placements.slice().sort((a, b) => (a.role === "Rejected") - (b.role === "Rejected") || a.layer_start - b.layer_start);
    $("#calc-table").style.setProperty("--cols", "1.1fr .8fr 1.5fr .6fr .8fr 1fr 1.6fr");
    $("#calc-table").innerHTML = `<div class="tr th"><span>device</span><span>layers</span><span>memory</span><span class="num">share</span><span class="num">gflop/tok</span><span class="num">ms/tok</span><span>verdict</span></div>` +
      rows.map(p => {
        const used = p.role !== "Rejected";
        const layers = used ? `${p.layer_start}–${Math.max(p.layer_start, p.layer_end - 1)} (${p.layer_end - p.layer_start})` : "—";
        const mem = used ? `${gb(p.bytes)} <span class="muted xs">+ ${gb(p.compute_reserve_bytes)} reserve (estimate)</span>` : "—";
        const share = used && plan.total_needed_bytes ? `${Math.round(100 * (p.bytes + p.compute_reserve_bytes) / plan.total_needed_bytes)}%` : "—";
        const s = used ? stacksById[p.device_id] : null;
        const gflop = s ? fmt(s.gflop_per_token, 1) : "—";
        const msTok = s && s.decode_ms.src !== "unknown" ? `${fmt(s.decode_ms.v, 0)}${dtag(srcTag(s.decode_ms.src))}` : (s ? "unknown" : "—");
        const verdict = used ? `capable · ${p.role === "Host" ? "host" : "helper"}, in use` : plainReason(p.reason);
        return `<div class="tr"><span>${esc(p.name)}</span><span>${layers}</span><span>${mem}</span><span class="num">${share}</span><span class="num">${gflop}</span><span class="num">${msTok}</span><span>${esc(verdict)}</span></div>`;
      }).join("");
  }
  function renderCalcTime(pred, qout) {
    if (!pred || pred.unknown) { $("#calc-time").innerHTML = `<div class="callout">unknown — run this plan once to measure.</div>`; return; }
    const t = dtag(pred.tag);
    $("#calc-time").innerHTML = `<div class="grid3">
        <div class="stat"><div class="k">first word</div><div class="v">${fmtSecs(pred.ttftMs / 1000)}${t}</div><div class="muted xs range">${fmtSecs(pred.ttftLo / 1000)}–${fmtSecs(pred.ttftHi / 1000)}</div></div>
        <div class="stat"><div class="k">speed</div><div class="v">${fmt(pred.tokS, 1)}<small>tok/s</small>${t}</div><div class="muted xs range">${fmt(pred.tokSLo, 1)}–${fmt(pred.tokSHi, 1)} tok/s</div></div>
        <div class="stat"><div class="k">whole answer (~${qout} words)</div><div class="v">${fmtSecs(pred.totalMs / 1000)}${t}</div><div class="muted xs range">${fmtSecs(pred.totalLo / 1000)}–${fmtSecs(pred.totalHi / 1000)}</div></div>
      </div>${pred.note ? `<div class="muted sm" style="margin-top:8px">${esc(pred.note)}</div>` : ""}`;
  }
  function renderCalcHistory(history) {
    const el = $("#calc-history"); if (!el) return;
    el.textContent = history ? `measured last time: ${fmt(history.tok_s, 1)} tok/s, ${fmt(history.ttft_ms, 0)} ms to first word (${new Date(history.when_ms).toLocaleString()})` : "";
  }
  function renderBottleneck(prediction) {
    const el = $("#calc-bottleneck"); if (!el) return;
    el.textContent = (prediction && prediction.bottleneck) ? `limited by: ${prediction.bottleneck}` : "";
  }
  function renderCalcWarnings(warnings) {
    const wrap = $("#calc-warnings-wrap"); if (!wrap) return;
    if (!warnings || !warnings.length) { wrap.hidden = true; $("#calc-warnings").innerHTML = ""; return; }
    wrap.hidden = false;
    $("#calc-warnings").innerHTML = warnings.map(w => `<div class="sm muted">${esc(w)}</div>`).join("");
  }
  /* Server-computed "why not the phone too?" (CalcResponse.alternatives) — a split the planner
     did not choose, costed anyway so the panel can say why not (§17.4). */
  function renderAlternatives(alternatives) {
    const el = $("#calc-whynot");
    if (!alternatives || !alternatives.length) { el.innerHTML = ""; return; }
    el.innerHTML = alternatives.map(a => {
      const ts = a.prediction && a.prediction.tok_s;
      const speed = ts && ts.src !== "unknown" ? `${fmt(ts.v, 1)} tok/s${dtag(srcTag(ts.src))}` : "speed unknown";
      return `<div class="callout">Why not <b>${esc(a.label)}</b>? ${speed} — ${esc(a.why_not)}</div>`;
    }).join("");
  }
  /* Fallback only (see timeEstimate above): the old client-side reasoning from plan.placements,
     used only when /api/calculate itself could not be reached. */
  function renderWhyNotFallback(plan) {
    const el = $("#calc-whynot");
    if (plan.mode !== "Single") { el.innerHTML = ""; return; }
    const phone = online().find(d => !d.is_local && kindOf(d) !== "sim");
    const rej = phone && plan.placements.find(p => p.device_id === phone.id && p.role === "Rejected");
    el.innerHTML = rej ? `<div class="callout">Why not the phone too? <b>${esc(phone.name)}</b>: ${esc(plainReason(rej.reason))}</div>` : "";
  }
  function renderVerdict() {
    const el = $("#verdict-wrap");
    if (CALC_FALLBACK) {
      const note = `<div class="callout">meshd's calculator (/api/calculate) is not reachable right now (${esc(CALC_CONN_ERROR)}) — showing a rough client-side estimate instead.</div>`;
      if (CALC_PLAN_ERROR) { el.innerHTML = note + `<div class="verdict no">Cannot run: ${esc(CALC_PLAN_ERROR)}<div class="hint">Try this: ${esc(fixHint(CALC_PLAN_ERROR))}</div></div>`; return; }
      if (!CALC_PLAN) { el.innerHTML = note; return; }
      const plan = CALC_PLAN; const used = plan.placements.filter(p => p.role !== "Rejected").length;
      const headline = plan.mode === "Single" ? `Can run on ${esc(devName(plan.host_id))} alone` : `Can run split across ${used} devices`;
      el.innerHTML = note + `<div class="verdict ${plan.mode === "Single" ? "ok" : "split"}">${headline}<div class="num-line">${esc(plan.summary)}</div></div>`;
      return;
    }
    if (!CALC_RESP) { el.innerHTML = ""; return; }
    const cls = CALC_RESP.verdict_code === "can_run_single" ? "ok" : CALC_RESP.verdict_code === "can_run_split" ? "split" : "no";
    const hints = (CALC_RESP.fix_hints || []).length ? `<div class="hint">Try this: ${esc(CALC_RESP.fix_hints.join(" · "))}</div>` : "";
    el.innerHTML = `<div class="verdict ${cls}">${esc(CALC_RESP.verdict)}${CALC_PLAN ? `<div class="num-line">${esc(CALC_PLAN.summary)}</div>` : ""}${hints}</div>`;
  }
  const calcKey = () => `${SELECTED}|${$("#qs-ctx").value}`;
  async function doCalculate() {
    if (!SELECTED) return toast("pick a model first (step 2)");
    if (CALC_BUSY) return;
    CALC_BUSY = true;
    $("#calc-btn").disabled = true;
    const qin = Math.max(1, +$("#q-in").value || 500), qout = Math.max(1, +$("#q-out").value || 200);
    const ctx = +$("#qs-ctx").value;
    CALC_FALLBACK = false; CALC_CONN_ERROR = null; CALC_PLAN_ERROR = null;
    try {
      const r = await api("/api/calculate", { method: "POST", body: JSON.stringify({ model: SELECTED, n_ctx: ctx, host: null, prompt_tokens: qin, answer_tokens: qout }) });
      CALC_RESP = r; CALC_PLAN = r.plan; PREDICTED = normalizePrediction(r.prediction);
      // Args for the Details drawer's "llama.cpp invocation" log; /api/calculate does not carry
      // them, so a cheap second local call fills that dev-only display when a plan exists. Never
      // blocks the render, and skipped entirely when refused (there is no plan to build args for).
      if (r.plan) { try { const pr = await api("/api/plan", { method: "POST", body: JSON.stringify({ model: SELECTED, n_ctx: ctx, host: null }) }); CALC_ARGS = pr.args; } catch { CALC_ARGS = null; } }
      else { CALC_ARGS = null; }
    } catch (e) {
      CALC_FALLBACK = true; CALC_CONN_ERROR = e.message; CALC_RESP = null;
      try { RUNS = await api("/api/runs"); } catch {}
      try {
        const r = await api("/api/plan", { method: "POST", body: JSON.stringify({ model: SELECTED, n_ctx: ctx, host: null }) });
        CALC_PLAN = r.plan; CALC_ARGS = r.args;
        PREDICTED = normalizeFallback(timeEstimate(CALC_PLAN, qin, qout));
      } catch (e2) { CALC_PLAN = null; CALC_ARGS = null; PREDICTED = null; CALC_PLAN_ERROR = e2.message; }
    }
    CALC_KEY_DONE = calcKey();
    CALC_BUSY = false;
    $("#calc-btn").disabled = false;
    renderPrepare();
  }
  $("#calc-btn").onclick = doCalculate;
  $("#run-btn").onclick = async () => {
    if (!CALC_PLAN) return;
    try { await api("/api/run", { method: "POST", body: JSON.stringify({ model: SELECTED, n_ctx: +$("#qs-ctx").value, host: null }) }); toast("starting…"); poll(); }
    catch (e) { toast(e.message); }
  };

  // Readiness checklist (requirement 4): model on disk, devices online, memory ok (from the
  // /api/calculate verdict), link ok — computed from whatever Calculate has already produced.
  function readinessItems() {
    const onDisk = !!(SELECTED && S.models.some(m => m.file === SELECTED));
    const plan = CALC_PLAN;
    const used = plan ? plan.placements.filter(p => p.role !== "Rejected") : [];
    const devicesOnline = plan ? used.length > 0 && used.every(p => (devById(p.device_id) || {}).online) : false;
    const memOk = CALC_RESP ? (CALC_RESP.verdict_code === "can_run_single" || CALC_RESP.verdict_code === "can_run_split") : (CALC_FALLBACK ? !!CALC_PLAN && !CALC_PLAN_ERROR : false);
    const phoneWorkers = used.filter(p => { const d = devById(p.device_id); return d && !d.is_local; });
    const linkOk = plan ? (phoneWorkers.length ? phoneWorkers.every(p => (((devById(p.device_id) || {}).telemetry || {}).rtt_ms_p95 || 0) <= 60) : true) : false;
    return [
      { ok: onDisk, text: "model on disk" },
      { ok: devicesOnline, text: "devices online" },
      { ok: memOk, text: "memory ok" },
      { ok: linkOk, text: "link ok" },
    ];
  }
  function renderPrepareChecklist() {
    const el = $("#prepare-checklist"); if (!el) return;
    el.innerHTML = readinessItems().map(i => `<div class="ck-item ${i.ok ? "ok" : ""}"><b></b>${esc(i.text)}</div>`).join("");
  }
  function renderPrepareLaptop() {
    const el = $("#prepare-laptop"); if (!el) return;
    if (!SELECTED) { el.innerHTML = ""; return; }
    const onDisk = S.models.find(m => m.file === SELECTED);
    const dl = (S.downloads || []).find(d => d.file === SELECTED);
    let html = `<div class="dv-h">on this laptop</div>`;
    if (onDisk) {
      html += `<div class="muted sm">on disk · ${gb(onDisk.info.file_bytes)}</div>`;
    } else if (dl && !dl.done) {
      const secs = Math.max(1, ((S.now_ms || Date.now()) - dl.started_ms) / 1000); const rate = dl.bytes / secs; const left = rate > 0 ? (dl.total - dl.bytes) / rate : null;
      html += `<div class="muted sm">downloading: ${(dl.bytes / 1e6).toFixed(0)} of ${(dl.total / 1e6).toFixed(0)} MB · ${(rate / 1e6).toFixed(1)} MB/s${left != null ? ` · about ${fmtEta(left)} left` : ""}</div>`;
      const resumedFrom = DL_INITIAL[SELECTED] || 0;
      if (resumedFrom > 1e6) html += `<div class="muted xs">continuing from ${(resumedFrom / 1e6).toFixed(0)} MB</div>`;
    } else {
      const cat = catOf(SELECTED);
      html += `<div class="muted sm">not downloaded yet${cat ? ` · ${gb(cat.bytes)}` : ""}</div><button class="btn sm" id="prepare-dl-btn"><i data-i="download"></i>Download</button>`;
    }
    el.innerHTML = html; drawIcons(el);
    const btn = $("#prepare-dl-btn");
    if (btn) btn.onclick = async () => {
      const cat = catOf(SELECTED); if (!cat) return toast("no catalog entry for this file — use Details → Model files");
      try { await api("/api/models/download", { method: "POST", body: JSON.stringify({ id: cat.id }) }); toast("download started"); poll(); } catch (e) { toast(e.message); }
    };
  }
  function renderPreparePhones() {
    const el = $("#prepare-phones"); if (!el) return;
    const plan = CALC_PLAN;
    const phoneWorkers = plan ? plan.placements.filter(p => p.role !== "Rejected").map(p => devById(p.device_id)).filter(d => d && !d.is_local) : [];
    el.innerHTML = phoneWorkers.length ? `<div class="dv-h">on each phone that will hold layers</div>` + phoneWorkers.map(d => `<div class="muted sm">${esc(d.name)}: layers will be sent when the run starts; cached from last time if unchanged</div>`).join("") : "";
  }
  function renderPrepare() {
    const mi = SELECTED ? S.models.find(m => m.file === SELECTED) : null;
    const cat = SELECTED ? catOf(SELECTED) : null;
    const label = mi ? (mi.info.name || SELECTED) : (cat ? cat.name : (SELECTED || "no model chosen yet — go back to step 2"));
    $("#prepare-model-name").textContent = label;
    $("#calc-model-name").textContent = label;
    if (SELECTED && calcKey() !== CALC_KEY_DONE) doCalculate();
    renderPrepareChecklist();
    renderPrepareLaptop();
    renderPreparePhones();
    const stale = calcKey() !== CALC_KEY_DONE;
    if (stale) {
      $("#verdict-wrap").innerHTML = CALC_KEY_DONE ? `<div class="callout">Model or context changed since the last Calculate — recalculating…</div>` : "";
      $("#calc-detail").hidden = true;
      return;
    }
    renderVerdict();
    $("#calc-detail").hidden = !CALC_PLAN;
    if (CALC_PLAN) {
      $("#calc-stack").innerHTML = renderCalcStack(CALC_PLAN);
      renderCalcTable(CALC_PLAN, CALC_RESP && CALC_RESP.prediction);
      renderCalcTime(PREDICTED, Math.max(1, +$("#q-out").value || 200));
      renderCalcHistory(CALC_RESP && CALC_RESP.history);
      renderBottleneck(CALC_RESP && CALC_RESP.prediction);
      if (CALC_FALLBACK) renderWhyNotFallback(CALC_PLAN); else renderAlternatives(CALC_RESP.alternatives);
      renderCalcWarnings(CALC_RESP && CALC_RESP.warnings);
    }
  }

  /* ---- step 4: use (chat) ---- */
  function renderPredicted() {
    const el = $("#predicted-vs-measured");
    if (!LAST_CHAT) { el.textContent = ""; return; }
    if (!PREDICTED || PREDICTED.unknown) { el.textContent = "no prediction was available before this run."; return; }
    el.innerHTML = `predicted before running: ${fmtSecs(PREDICTED.ttftMs / 1000)} to first word, ${fmt(PREDICTED.tokS, 1)} tok/s${dtag(PREDICTED.tag)} · measured just now: ${fmt(LAST_CHAT.ttft / 1000, 1)} s, ${fmt(LAST_CHAT.tps, 1)} tok/s${dtag("m")}`;
  }

  /* ---- Details drawer content ---- */
  function renderPlanArgsLog() {
    const args = (S.run && S.run.args && S.run.args.length) ? S.run.args : (CALC_ARGS ? [CALC_ARGS.program, ...CALC_ARGS.args] : []);
    $("#plan-args").textContent = args.join(" ");
    const r = S.run || {};
    $("#log").textContent = (r.log_tail || []).join("\n") + (r.error ? "\nERROR: " + r.error : "");
  }
  function renderFeed() {
    const r = S.run || {}; const lines = (r.log_tail || []).slice(-30).reverse();
    const cls = l => /error|failed|✗|exited/i.test(l) ? "err" : /ready|listening|paired|credits|serving/i.test(l) ? "ok" : /download|fetch|push|plan|host:/i.test(l) ? "io" : "";
    const extra = S.devices.filter(d => d.progress && (Date.now() - d.progress.ms) < 120000).map(d => `<div class="line io"><b></b><span>${esc(d.name)}: ${esc(d.progress.job)} ${Math.round(d.progress.fraction * 100)}%</span></div>`).join("");
    $("#feed").innerHTML = extra + lines.map(l => `<div class="line ${cls(l)}"><b></b><span>${esc(l.length > 140 ? l.slice(0, 140) + "…" : l)}</span></div>`).join("") || '<div class="muted sm">nothing yet</div>';
  }
  function renderAdvancedModels() {
    const onDisk = new Set(S.models.map(m => m.file)); const dl = f => (S.downloads || []).find(d => d.file === f);
    $("#catalog").innerHTML = CAT.map(c => { const d = dl(c.file); const have = onDisk.has(c.file); const pct = d && d.total ? Math.round(100 * d.bytes / d.total) : 0;
      return `<div class="model"><div class="model-h"><div class="mark">${esc((c.provider || "M")[0].toUpperCase())}</div><div><div class="m-name">${esc(c.name)}</div><div class="m-sub">${esc(c.file)}</div></div></div><div class="m-role">${esc(c.role)} · ${gb(c.bytes)}${c.layers ? ` · ${c.layers} layers` : ""}</div><div class="muted sm">${esc(c.note)}</div>${d && !d.done ? `<div class="progress"><i style="width:${pct}%"></i></div><div class="muted xs mono">${gb(d.bytes)} / ${gb(d.total)} · ${pct}%</div>` : ""}<div class="row">${have ? `<span class="tag">on disk</span>` : `<button class="btn sm" data-dl="${c.id}" ${d && !d.done ? "disabled" : ""}>${svg("download")} ${d && !d.done ? "Downloading…" : "Download"}</button>`}</div></div>`; }).join("");
    $$("[data-dl]", $("#catalog")).forEach(b => b.onclick = async () => { try { await api("/api/models/download", { method: "POST", body: JSON.stringify({ id: b.dataset.dl }) }); toast("download started"); poll(); } catch (e) { toast(e.message); } });
    $("#disk").style.setProperty("--cols", "2fr 1fr 1fr 1fr 1fr");
    $("#disk").innerHTML = `<div class="tr th"><span>file</span><span>arch</span><span>quant</span><span class="num">size</span><span class="num">layers</span></div>` + S.models.map(m => `<div class="tr"><span class="mono">${esc(m.file)}</span><span>${esc(m.info.arch)}</span><span>${esc(m.info.quant_label)}</span><span class="num">${gb(m.info.file_bytes)}</span><span class="num">${m.info.n_layer}</span></div>`).join("");
  }
  function renderRuns() {
    const by = {}; for (const r of RUNS) { if (!r.tokens_out) continue; const k = `${r.model} · ${r.mode} · ${r.devices} dev · host ${devName(r.host)}`; (by[k] ||= []).push(r.tps); }
    const agg = Object.entries(by).map(([k, v]) => [k, v.reduce((a, b) => a + b, 0) / v.length, v.length]).sort((a, b) => b[1] - a[1]); const max = Math.max(1, ...agg.map(a => a[1]));
    $("#runs-bars").innerHTML = agg.map(([k, v, n]) => `<div class="bar"><span title="${esc(k)}">${esc(k)} <span class="muted xs">×${n}</span></span><div class="track"><i style="width:${100 * v / max}%"></i></div><span class="num">${fmt(v, 1)} tok/s</span></div>`).join("") || '<div class="muted sm">no requests yet</div>';
    const ok = RUNS.filter(r => r.ok && r.tokens_out > 0); const avg = f => ok.length ? ok.reduce((a, r) => a + f(r), 0) / ok.length : NaN;
    $("#runs-stats").innerHTML = [["Requests", RUNS.length, ""], ["Avg decode", fmt(avg(r => r.tps), 1), "tok/s"], ["Avg TTFT", fmt(avg(r => r.ttft_ms), 0), "ms"]].map(([k, v, u]) => `<div class="stat"><div class="k">${k}</div><div class="v">${v}<small>${u}</small></div></div>`).join("");
    $("#runs-table").style.setProperty("--cols", "1.2fr 2fr 1fr 1fr .7fr .7fr .7fr");
    $("#runs-table").innerHTML = `<div class="tr th"><span>time</span><span>model</span><span>mode</span><span>host</span><span class="num">TTFT</span><span class="num">tok</span><span class="num">tok/s</span></div>` + RUNS.slice().reverse().slice(0, 100).map(r => `<div class="tr"><span class="mono xs">${new Date(r.ts_ms).toLocaleTimeString()}</span><span>${esc(r.model)}</span><span>${esc(r.mode)}</span><span>${esc(devName(r.host))}</span><span class="num">${r.ttft_ms} ms</span><span class="num">${r.tokens_out}</span><span class="num">${fmt(r.tps, 1)}</span></div>`).join("");
  }

  /* ---- use: chat (appears only once a run has started) ---- */
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
    $("#use-model-name").textContent = r.status === "ready" ? (r.model || "").replace(/\.gguf$/, "") : "starting…";
    const cards = [
      ["Model", r.status === "ready" ? r.model.replace(/\.gguf$/, "") : "starting…", r.status === "ready" ? (vis ? "text + images" : "text only") : "", r.status === "ready" ? "accent" : ""],
      ["Memory for models", gb(pooled()), `${online().length} device${online().length === 1 ? "" : "s"} online`, ""],
      ["Laptop", lap ? `${(lap.profile.cores || []).length} cores` : "—", lap ? gb(lap.usable_bytes) + " free for models" : "", S.plan && S.plan.host_id === "local" ? "host" : ""],
      ["Phone", p ? `${(p.profile && p.profile.cores || []).length} cores` : "not connected", p ? `${gb(p.usable_bytes)} free · link ${fmt((p.telemetry || {}).rtt_ms_p50, 1)} ms` : "", p && S.plan && S.plan.placements.some(x => x.device_id === p.id && x.role !== "Rejected") ? (S.plan.host_id === p.id ? "host" : "worker") : ""],
      ["Last answer", LAST_CHAT ? `${fmt(LAST_CHAT.tps, 1)} tok/s` : "—", LAST_CHAT ? `${fmt(LAST_CHAT.ttft, 0)} ms to first word` : "", ""],
    ];
    $("#power").innerHTML = cards.map(([k, v, s, cls]) => `<div class="pw ${cls}"><div class="k">${esc(k)}</div><div class="v">${esc(v)}</div><div class="s">${esc(s)}</div></div>`).join("");
    $("#cando").innerHTML = CANDO.map(([t, q, need]) => { const dis = (need === "image" && !vis); return `<button data-q="${esc(q)}" data-need="${need}" class="${dis ? "needs" : ""}" title="${dis ? "needs a vision model (step 2: Qwen2.5-VL)" : q}">${esc(t)}</button>`; }).join("");
    $$("#cando button").forEach(b => b.onclick = () => { $("#chat-input").value = b.dataset.q; if (b.dataset.need) { $("#chat-file").click(); } else { $("#chat-input").focus(); } });
    $("#chat-model").textContent = r.status === "ready" ? `${r.model.replace(/\.gguf$/, "")} on ${devName(r.host_id)}${vis ? " · can see images" : ""}` : "loading…";
    const used = S.plan ? S.plan.placements.filter(x => x.role !== "Rejected") : [];
    $("#who").innerHTML = used.length ? used.map(x => { const d = devById(x.device_id) || { kind: "phone" }; return `<div class="w ${x.role === "Host" ? "host" : "worker"}">${DEV_ICON[kindOf(d)]}<div>${esc(x.name)}<br><span class="muted xs">${x.role === "Host" ? "writes the answer" : "computes layers " + x.layer_start + "–" + Math.max(x.layer_start, x.layer_end - 1)}</span></div></div>`; }).join("") : '<div class="muted sm">no model running</div>';
  }

  function render() {
    renderNow();
    renderRecovery();
    renderStepNav();
    if (STEP === "devices") { renderDevices(); renderUsb(); renderAdvice(); renderMeshSummary(); }
    else if (STEP === "models") { renderModels(); }
    else if (STEP === "prepare") { renderPrepare(); }
    else { renderPower(); renderPredicted(); }
    renderPlanArgsLog();
    renderFeed();
    if (DETAILS_OPEN) { renderAdvancedModels(); renderRuns(); }
  }

  /* ---- actions ---- */
  const stop = async () => { try { await api("/api/stop", { method: "POST" }); toast("stopped"); poll(); } catch (e) { toast(e.message); } };
  $("#stop-btn").onclick = stop;
  $("#stop-back-btn").onclick = async () => { await stop(); showStep("prepare"); };
  $("#offer-btn").onclick = async () => { try { OFFER = await api("/api/pair/offer", { method: "POST" }); renderDevices(); } catch (e) { toast(e.message); } };
  $("#sim-btn").onclick = async () => { try { await api("/api/sim/workers", { method: "POST", body: JSON.stringify({ n: +$("#sim-n").value, usable_gb: +$("#sim-gb").value, spawn: $("#sim-spawn").checked }) }); toast(`simulated ${$("#sim-n").value} phone(s)`); poll(); } catch (e) { toast(e.message); } };
  $("#dl-btn").onclick = async () => { try { await api("/api/models/download", { method: "POST", body: JSON.stringify({ url: $("#dl-url").value, file: $("#dl-file").value }) }); toast("download started"); poll(); } catch (e) { toast(e.message); } };
  $("#rescan").onclick = async () => { await api("/api/models/rescan", { method: "POST" }); poll(); };
  $("#demo-split-btn").onclick = async () => {
    try {
      await api("/api/devices/local/limit", { method: "POST", body: JSON.stringify({ usable_gb: 4 }) });
      $("#demo-note").textContent = "laptop capped to 4 GB. Go to step 2, pick Qwen3-8B if not already, then step 3 and press Calculate.";
      toast("laptop capped to 4 GB");
      poll(); if (STEP === "prepare" && CALC_KEY_DONE) doCalculate();
    } catch (e) { toast(e.message); }
  };
  $("#demo-uncap-btn").onclick = async () => {
    try { await api("/api/devices/local/limit", { method: "POST", body: JSON.stringify({ usable_gb: null }) }); $("#demo-note").textContent = "cap removed."; toast("cap removed"); poll(); if (STEP === "prepare" && CALC_KEY_DONE) doCalculate(); }
    catch (e) { toast(e.message); }
  };

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
    if ((S.run || {}).status !== "ready") return toast("wait for the model to finish loading");
    const images = ATT.filter(a => a.kind === "image"), texts = ATT.filter(a => a.kind === "text");
    if (images.length && !isVision(S.run.model)) return toast("this model cannot see images — pick Qwen2.5-VL in step 2");
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
      renderPredicted();
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
    try { const s = localStorage.getItem("meshai-step"); if (STEP_ORDER.includes(s)) STEP = s; } catch {}
    await poll(); await pollUsb(); showStep(STEP);
    setInterval(poll, 2000); setInterval(pollUsb, 5000);
  })();
})();
