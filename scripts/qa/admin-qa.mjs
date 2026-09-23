// Drives the MeshAI admin panel like a user: pick model → Preview plan → Run → wait ready → Chat → read timings → Analytics.
// Usage: node ~/.claude/skills/browser-automation/browser.mjs http://localhost:8080/admin/ --script scripts/qa/admin-qa.mjs  (Playwright page + ref helper; run split-sim.sh first)
export default async function run(page, ui) {
  const out = { steps: [] };
  const step = (n, v) => out.steps.push({ [n]: v });
  const q = (sel) => page.locator(sel);
  await page.waitForSelector('#qs-model option', { state: 'attached', timeout: 15000 });

  // Overview: choose the 0.6B model, small context, preview plan
  const model = (await q('#qs-model option').allTextContents()).find(t => t.includes('0.6B')) ? await q('#qs-model option').filter({ hasText: '0.6B' }).first().getAttribute('value') : null;
  if (!model) return { error: 'no 0.6B model in select', options: await q('#qs-model option').allTextContents() };
  await q('#qs-model').selectOption(model);
  await q('#qs-ctx').selectOption('2048');
  await q('#qs-plan').click();
  await page.waitForFunction(() => document.querySelector('#qs-result').innerText.length > 20, null, { timeout: 15000 });
  step('plan_preview', (await q('#qs-result').innerText()).slice(0, 400));

  // Run and wait for ready
  await q('#qs-run').click();
  await page.waitForFunction(() => ['ready', 'error'].includes(document.querySelector('#status-pill').className.replace('status ', '')), null, { timeout: 180000 });
  step('status', await q('#status-text').innerText());
  step('topology_nodes', await q('#topology .node').count());
  step('stats', (await q('#ov-stats').innerText()).replace(/\n/g, ' | '));

  // Plan view
  await q('#nav button[data-view=plan]').click();
  await page.waitForFunction(() => document.querySelectorAll('#layer-map > div').length > 0, null, { timeout: 5000 });
  step('layer_map', await q('#layer-map > div').allTextContents());
  step('plan_args', (await q('#plan-args').innerText()).slice(0, 300));

  // Chat
  await q('#nav button[data-view=chat]').click();
  await q('#chat-input').fill('In one sentence, what is a compute mesh?');
  await q('#chat-send').click();
  await page.waitForFunction(() => document.querySelector('#s-tps').innerText !== '—', null, { timeout: 240000 });
  step('chat_reply', (await q('#chat .msg.assistant').last().innerText()).slice(0, 500));
  step('chat_stats', { ttft: await q('#s-ttft').innerText(), tps: await q('#s-tps').innerText(), tokens: await q('#s-tok').innerText(), total: await q('#s-total').innerText() });

  // Analytics
  await q('#nav button[data-view=runs]').click();
  await page.waitForFunction(() => document.querySelectorAll('#runs-table .tr').length > 1, null, { timeout: 10000 });
  step('analytics_stats', (await q('#runs-stats').innerText()).replace(/\n/g, ' | '));
  step('bars', (await q('#runs-bars').innerText()).replace(/\n/g, ' | ').slice(0, 400));

  // Devices + Models views render
  await q('#nav button[data-view=devices]').click();
  step('device_cards', await q('#devices .dev').count());
  await q('#offer-btn').click();
  await page.waitForFunction(() => document.querySelector('#qr svg') !== null, null, { timeout: 10000 });
  step('qr_rendered', await q('#qr svg').count());
  await q('#nav button[data-view=models]').click();
  step('catalog_cards', await q('#catalog .model').count());
  step('disk_rows', await q('#disk .tr').count());
  await page.screenshot({ path: '/tmp/claude-1000/admin-models.png', fullPage: false });
  await q('#nav button[data-view=chat]').click();
  await page.screenshot({ path: '/tmp/claude-1000/admin-chat.png', fullPage: false });
  return out;
}
