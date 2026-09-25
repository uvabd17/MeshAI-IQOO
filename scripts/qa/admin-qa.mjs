// Drives the MeshAI admin panel's four-step guided flow (Devices / Models / Prepare / Use) like a
// first-time user and checks the server-computed verdicts, never a client-side guess — plus the
// gating rules from the product-flow audit (T095/T096/T098, docs/MESHAI.md §20).
//
// Usage: node ~/.claude/skills/browser-automation/browser.mjs <base-url> --script scripts/qa/admin-qa.mjs [--session s]
// Env:
//   MESHAI_QA_URL    base URL of the meshd instance to test, e.g. http://127.0.0.1:8090 (default: the <base-url> argument)
//   MESHAI_QA_TOKEN  x-mesh-token / ?token= value (default: "qa")
//   MESHAI_QA_SHOTS  directory to write screenshots into (default: system tmp dir)
//   MESHAI_QA_MODEL  substring of the model file to pick for the "fits alone" case (default: "Qwen3-8B")
//
// Precondition the caller is responsible for: meshd running at MESHAI_QA_URL with
// MESHAI_API_TOKEN set to MESHAI_QA_TOKEN. The script manages its own simulated-phone fixture
// (POST /api/sim/workers {n:1, usable_gb:2, spawn:false}, added only after the no-phone gating
// assertions below, so it never masks them) and removes it (DELETE /api/devices/sim-phone-1)
// along with any memory cap it set before returning — safe to re-run without manual cleanup.
// This script never presses Start and never spawns real processes.
import os from "node:os";

export default async function run(page, ui) {
  const BASE = (process.env.MESHAI_QA_URL || "http://127.0.0.1:8090").replace(/\/$/, "");
  const TOKEN = process.env.MESHAI_QA_TOKEN || "qa";
  const SHOTS = process.env.MESHAI_QA_SHOTS || os.tmpdir();
  const MODEL_SUBSTR = process.env.MESHAI_QA_MODEL || "Qwen3-8B";
  const out = { base: BASE, steps: [] };
  const step = (name, value) => { out.steps.push({ [name]: value }); };
  const shot = async name => { const p = `${SHOTS}/admin-${name}.png`; await page.screenshot({ path: p, fullPage: true }); step(name, p); };
  // Set <select> values directly via the DOM instead of page.selectOption: on this box (shared
  // with the user, run under `nice`, observed load average 2.8-5.1 with a qemu VM and a java
  // process each pegging a core) Playwright's actionability wait for a <select> intermittently
  // stalled past 90 s even though the element was found and enabled; a direct value set + change
  // event is what the app's own onchange handlers need and costs one JS tick, not a visibility poll.
  const setSelect = (sel, val) => page.evaluate(([s, v]) => { const el = document.querySelector(s); el.value = v; el.dispatchEvent(new Event("change")); }, [sel, val]);
  // Same reasoning as setSelect above, extended to every click in this script: a plain
  // page.click(selector) on this box was observed to hang past its 120 s timeout even though the
  // locator resolved immediately to a visible, enabled button (`waiting for locator(...)` /
  // `locator resolved to <button ...>` in the trace, then nothing) — Playwright's actionability
  // polling (visible + stable across two frames + receives pointer events) apparently never
  // settles under this box's load (nice-throttled, shared with the user, concurrent cargo builds
  // from another engineer's meshd work). A direct DOM .click() dispatch fires the same onclick
  // handler the app wires up without waiting on any of that, at the cost of not proving the
  // button was visually clickable — acceptable here since the assertions below check the
  // resulting state, not the click mechanics.
  const clickSel = sel => page.evaluate(s => { const el = document.querySelector(s); if (!el) throw new Error(`not found: ${s}`); el.click(); }, sel);
  page.setDefaultTimeout(120000); // this box is shared with the user and run under `nice`; be patient

  // Defensive: clear any leftover cap / laptop-only flag from a previous, possibly-interrupted run
  // before anything else, so the assertions below are not corrupted by stale state. In particular,
  // a leftover sim-phone-1 device would count as "a phone is paired" all by itself and defeat the
  // gate1 assertion below before this run has even added its own.
  await fetch(`${BASE}/api/devices/local/limit`, { method: "POST", headers: { "content-type": "application/json", "x-mesh-token": TOKEN }, body: JSON.stringify({ usable_gb: null }) }).catch(() => {});
  await fetch(`${BASE}/api/devices/sim-phone-1`, { method: "DELETE", headers: { "content-type": "application/json", "x-mesh-token": TOKEN } }).catch(() => {});

  // --- Gating, before any phone is paired: fresh localStorage, step 1 only ---
  await page.goto(`${BASE}/admin/?token=${encodeURIComponent(TOKEN)}`, { waitUntil: "domcontentloaded", timeout: 45000 });
  await page.evaluate(() => { try { localStorage.removeItem("meshai-laptop-only"); localStorage.removeItem("meshai-step"); localStorage.removeItem("meshai-selected-model"); } catch {} });
  await page.reload({ waitUntil: "domcontentloaded" });
  await page.waitForSelector("#devices2 .dv", { timeout: 90000 });
  step("gate0_landing_step", await page.evaluate(() => document.querySelector("#steps button.active")?.dataset.step));
  // Models must be locked before pairing / laptop-only: clicking it must NOT navigate there.
  await clickSel('#steps button[data-step="models"]');
  await page.waitForTimeout(400);
  const stillOnDevices = await page.evaluate(() => !document.querySelector("#step-devices").hidden && document.querySelector("#step-models").hidden);
  step("gate1_models_locked_before_pairing", stillOnDevices);
  step("gate1_models_button_has_locked_class", await page.evaluate(() => document.querySelector('#steps button[data-step="models"]').classList.contains("locked")));

  // --- "Continue with this laptop only" unlocks step 2 ---
  await clickSel("#laptop-only-btn");
  await page.waitForTimeout(300);
  await clickSel('#steps button[data-step="models"]');
  await page.waitForTimeout(500);
  const onModelsNow = await page.evaluate(() => document.querySelector("#steps button.active")?.dataset.step === "models");
  step("gate2_models_unlocked_after_laptop_only", onModelsNow);

  // Back to Devices: add the one simulated phone (T096's fixture) now that the "laptop only" path
  // has been proven on its own — a role chip (HOST / HELPER / NOT USABLE, requirement 2) must
  // appear on its card once it is online.
  await clickSel('#steps button[data-step="devices"]'); await page.waitForTimeout(300);
  await page.evaluate(async ({ base, token }) => {
    await fetch(`${base}/api/sim/workers`, { method: "POST", headers: { "content-type": "application/json", "x-mesh-token": token }, body: JSON.stringify({ n: 1, usable_gb: 2, spawn: false }) });
  }, { base: BASE, token: TOKEN });
  await page.waitForFunction(() => document.querySelectorAll("#devices2 .dv").length >= 2, { timeout: 15000 }).catch(() => {});
  await page.waitForTimeout(300);
  step("sim_phone_role_chip_rendered", await page.locator("#devices2 .dv .rolechip").count() > 0);
  await shot("step1-devices");

  // Prepare must still be locked: no model chosen yet.
  await clickSel('#steps button[data-step="prepare"]');
  await page.waitForTimeout(400);
  step("gate3_prepare_locked_before_model_choice", await page.evaluate(() => !document.querySelector("#step-models").hidden));

  // --- Step 2: Models — categories render (easy / hard / impossible), feasibility endpoint may
  // still be 404 on this meshd; either path (feasibility or the basic-fit fallback) must render
  // three non-crashing groups and show its own status honestly. ---
  await clickSel('#steps button[data-step="models"]'); await page.waitForTimeout(500);
  await page.waitForSelector("#cards-easy, #cards-hard, #cards-impossible", { timeout: 90000 });
  await page.waitForFunction(sub => {
    const groups = [...document.querySelectorAll("#cards-easy .mc, #cards-hard .mc, #cards-impossible .mc")];
    const b = groups.find(x => (x.dataset.model || "").includes(sub));
    return !!b || document.querySelector("#feasibility-note")?.textContent?.length > 0;
  }, MODEL_SUBSTR, { timeout: 90000 });
  step("step2_easy_count", await page.locator("#cards-easy .mc").count());
  step("step2_hard_count", await page.locator("#cards-hard .mc").count());
  step("step2_impossible_count", await page.locator("#cards-impossible .mc").count());
  step("step2_feasibility_note", await page.locator("#feasibility-note").innerText());
  await shot("step2-models");

  const modelBtn = page.locator(`.mc[data-model*="${MODEL_SUBSTR}"]`).first();
  if (await modelBtn.count() === 0) return { error: `no model card matches "${MODEL_SUBSTR}"`, ...out };
  await clickSel(`.mc[data-model*="${MODEL_SUBSTR}"]`);
  await setSelect("#qs-ctx", "4096");
  await page.waitForTimeout(300);

  // Choosing a model unlocks Prepare.
  await clickSel('#steps button[data-step="prepare"]');
  await page.waitForTimeout(500);
  step("gate4_prepare_unlocked_after_model_choice", await page.evaluate(() => document.querySelector("#steps button.active")?.dataset.step === "prepare"));

  // --- Step 3: Prepare (expect "fits alone" with the simulated phone present but not needed) ---
  await page.waitForSelector("#verdict-wrap .verdict", { timeout: 120000 });
  const verdictAlone = await page.locator("#verdict-wrap").innerText();
  const verdictAloneClass = await page.evaluate(() => document.querySelector("#verdict-wrap .verdict")?.className || "");
  step("step3_verdict_alone", verdictAlone.replace(/\n/g, " | "));
  step("step3_verdict_alone_class", verdictAloneClass);
  // Expected "fits alone" (verdict.ok), but this device's free memory is live and this box is
  // shared with the user; if something else is holding memory right now the honest answer can
  // legitimately be "split" instead (verdict.split) rather than an error (verdict.no). Either
  // ok or split proves the real /api/calculate verdict rendered and coloured correctly; only "no"
  // (or no banner at all) is treated as a failure here.
  step("step3_verdict_alone_rendered_ok", /\bverdict\s+(ok|split)\b/.test(verdictAloneClass));
  step("step3_checklist_rendered", await page.locator("#prepare-checklist .ck-item").count() > 0);
  // "Time for one question" tiles must show a source tag (m/d/e/?, docs/MESHAI.md §17.4) next to
  // each number — proof the prediction came from POST /api/calculate's tagged Est values, not an
  // untagged guess. Only asserted when the detail card actually rendered (a plan exists).
  if (await page.locator("#calc-detail").isVisible()) {
    const tileTagCount = await page.locator("#calc-time .dtag").count();
    step("step3_tiles_have_tag", tileTagCount > 0);
  }
  await shot("step3-prepare-alone");

  // Use must still be locked: the run was never started.
  await clickSel('#steps button[data-step="use"]'); await page.waitForTimeout(400);
  step("gate5_use_locked_before_ready", await page.evaluate(() => !document.querySelector("#step-prepare").hidden));

  // --- Step 3: a refused plan (cap the laptop to 1 GB, then ask for 16k context) shows a
  // recovery-style "Try this" hint, not just a bare "cannot run". ---
  // #devices2 re-renders on every 2 s poll (it must, to show live telemetry), which races a
  // typed .fill() into the cap input off the DOM mid-keystroke. Call the same endpoint the
  // input's onchange handler calls instead — this checks the calculate/verdict path this test
  // exists for, not the input widget's typing mechanics.
  await page.evaluate(async ({ base, token }) => {
    await fetch(`${base}/api/devices/local/limit`, { method: "POST", headers: { "content-type": "application/json", "x-mesh-token": token }, body: JSON.stringify({ usable_gb: 1 }) });
  }, { base: BASE, token: TOKEN });
  await page.waitForTimeout(300);
  step("cap_applied", true);

  await clickSel('#steps button[data-step="models"]'); await page.waitForTimeout(600);
  await setSelect("#qs-ctx", "16384");
  await clickSel('#steps button[data-step="prepare"]'); await page.waitForTimeout(600);
  await clickSel("#calc-btn");
  await page.waitForSelector("#verdict-wrap .verdict", { timeout: 120000 });
  const verdictRefused = await page.locator("#verdict-wrap").innerText();
  step("step3_verdict_refused", verdictRefused.replace(/\n/g, " | "));
  step("step3_verdict_refused_is_no", await page.locator("#verdict-wrap .verdict.no").count() > 0);
  const fixHintsText = await page.locator("#verdict-wrap .verdict.no .hint").innerText().catch(() => "");
  step("step3_fix_hints", fixHintsText);
  step("step3_fix_hints_shown", /try this/i.test(fixHintsText) && fixHintsText.length > "Try this:".length);
  await shot("step3-refused");

  // Leave the server in a clean state for whoever looks at it next (no Start was ever pressed):
  // remove the memory cap and the simulated phone. A leftover sim-phone-1 from an earlier,
  // interrupted run satisfies the "a phone is paired" gate all by itself (it is `online` and not
  // `is_local`) and silently invalidates the gate1 assertion above on the next run — this cleanup
  // is what keeps that assertion honest.
  await page.evaluate(async ({ base, token }) => {
    await fetch(`${base}/api/devices/local/limit`, { method: "POST", headers: { "content-type": "application/json", "x-mesh-token": token }, body: JSON.stringify({ usable_gb: null }) });
  }, { base: BASE, token: TOKEN });
  await setSelect("#qs-ctx", "4096");
  await fetch(`${BASE}/api/devices/sim-phone-1`, { method: "DELETE", headers: { "content-type": "application/json", "x-mesh-token": TOKEN } }).catch(() => {});

  out.ok = out.steps.some(s => s.gate1_models_locked_before_pairing)
    && out.steps.some(s => s.gate2_models_unlocked_after_laptop_only)
    && out.steps.some(s => s.sim_phone_role_chip_rendered)
    && out.steps.some(s => s.gate3_prepare_locked_before_model_choice)
    && out.steps.some(s => s.gate4_prepare_unlocked_after_model_choice)
    && out.steps.some(s => s.gate5_use_locked_before_ready)
    && out.steps.some(s => s.step3_verdict_alone_rendered_ok)
    && out.steps.some(s => s.step3_checklist_rendered)
    && out.steps.some(s => s.step3_verdict_refused_is_no)
    && out.steps.some(s => s.step3_fix_hints_shown);
  return out;
}
