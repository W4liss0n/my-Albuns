import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { mkdirSync, openSync, closeSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { aliveProcessInstances, waitForProcessInstance, processForestInstances, terminateProcessInstance } from './DevLifecycleProcessInstances.mjs';
import { createWebDriverClient, findFreeTcpPort, waitForHttp } from './GateWebDriver.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.resolve(process.argv[2] ?? path.join(root, '.scratch/command-window-isolation'));
const source = () => ({
  gitCommit: execFileSync('git.exe', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim(),
  dirty: execFileSync('git.exe', ['status', '--porcelain'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim() !== '',
});
const evidence = { schemaVersion: 1, gate: 'command-window-isolation', collectedAtUtc: new Date().toISOString(),
  sourceInputs: { initial: source(), final: null }, passed: false, cleanupCompleted: false, checks: [] };
mkdirSync(output, { recursive: true });
const edge = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(root, 'scripts/Resolve-EdgeWebDriver.ps1')], { encoding: 'utf8', windowsHide: true }));
evidence.browser = { version: edge.edgeVersion, driverVersion: edge.driverVersion };
const roots = [];
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
async function start(executable, args, name) {
  const fd = openSync(path.join(output, name + '.log'), 'w');
  const child = spawn(executable, args, { cwd: root, windowsHide: true, stdio: ['ignore', fd, fd] });
  closeSync(fd);
  try { roots.push(await waitForProcessInstance(child.pid, name)); }
  catch (error) { child.kill(); throw error; }
}
let request, session;
try {
  const port = await findFreeTcpPort(), driverPort = await findFreeTcpPort();
  await start(process.execPath, [path.join(root, 'node_modules/vite/bin/vite.js'), '--host', '127.0.0.1', '--port', String(port), '--strictPort'], 'vite');
  await start(edge.driverExecutable, ['--port=' + driverPort, '--host=127.0.0.1'], 'webdriver');
  await Promise.all([waitForHttp(`http://127.0.0.1:${port}`, 'Vite'), waitForHttp(`http://127.0.0.1:${driverPort}/status`, 'WebDriver')]);
  request = createWebDriverClient(`http://127.0.0.1:${driverPort}`, { defaultTimeoutMilliseconds: 60000 });
  session = (await request('POST', '/session', { capabilities: { alwaysMatch: { browserName: 'MicrosoftEdge', 'ms:edgeOptions': {
    binary: edge.edgeExecutable, args: ['--headless=new', '--disable-gpu', '--no-first-run', '--window-size=1440,900'],
  } } } })).sessionId;
  const endpoint = `/session/${session}`;
  const execute = (script, args = []) => request('POST', `${endpoint}/execute/sync`, { script, args });
  const switchWindow = handle => request('POST', `${endpoint}/window`, { handle });
  async function until(read, accept, description) {
    const deadline = Date.now() + 60000;
    let last;
    do {
      last = await read();
      if (accept(last)) return last;
      await delay(100);
    } while (Date.now() < deadline);
    assert.fail(`${description}: ${JSON.stringify(last)}`);
  }
  const state = () => execute(`return {
    editing: document.querySelector('.canvas-host canvas')?.getAttribute('aria-label').startsWith('Canvas da Lâmina em edição') ?? null,
    count: document.body.dataset.clipboardCount ?? null,
    selection: document.body.dataset.clipboardSelection ?? null,
    clipboard: document.body.dataset.clipboardAvailable ?? null,
    title: document.querySelector('header')?.textContent ?? null
  };`);
  async function navigate(suffix) {
    const chrome = await execute('return { width: outerWidth - innerWidth, height: outerHeight - innerHeight };');
    await request('POST', `${endpoint}/window/rect`, { width: 1567 + chrome.width, height: 900 + chrome.height });
    await request('POST', `${endpoint}/url`, { url: `http://127.0.0.1:${port}/workspace-preview.html${suffix}` });
    await until(state, value => value.editing !== null, 'Canvas ready');
  }
  async function key(value, control = false, shift = false) {
    const modifiers = [...(control ? ['\uE009'] : []), ...(shift ? ['\uE008'] : [])];
    await request('POST', `${endpoint}/actions`, { actions: [{ type: 'key', id: 'commands', actions: [
      ...modifiers.map(value => ({ type: 'keyDown', value })),
      { type: 'keyDown', value }, { type: 'keyUp', value },
      ...modifiers.reverse().map(value => ({ type: 'keyUp', value })),
    ] }] });
  }
  const focusCanvas = () => execute("document.querySelector('.canvas-host canvas').focus();");
  const screenshot = async name => writeFileSync(path.join(output, `${name}.png`), Buffer.from(await request('GET', `${endpoint}/screenshot`), 'base64'));
  const first = await request('GET', `${endpoint}/window`);
  await navigate('?frame=clipboard&clipboard=same-group');
  const firstBefore = await state();
  assert.equal(firstBefore.editing, true);
  assert.equal(firstBefore.clipboard, 'false');
  const second = (await request('POST', `${endpoint}/window/new`, { type: 'window' })).handle;
  await switchWindow(second);
  await navigate('');
  const secondBefore = await state();
  assert.equal(secondBefore.editing, false);
  assert.notEqual(firstBefore.title, secondBefore.title, 'The two fixtures must represent different Projects');

  await switchWindow(first);
  await focusCanvas();
  await key('c', true);
  await until(state, value => value.clipboard === 'true', 'Copy in first Project');
  await focusCanvas();
  await key('v', true);
  const pasted = await until(state, value => Number(value.count) > Number(firstBefore.count), 'Paste in first Project');
  await switchWindow(second);
  assert.deepEqual(await state(), secondBefore, 'Copy/Paste must not reach the second Project');
  evidence.checks.push({ name: 'copy-paste-isolated', passed: true });

  await switchWindow(first);
  await focusCanvas();
  await key('z', true);
  await until(state, value => value.count === firstBefore.count, 'Undo in first Project');
  await key('z', true, true);
  await until(state, value => value.count === pasted.count, 'Redo in first Project');
  await switchWindow(second);
  assert.deepEqual(await state(), secondBefore, 'Undo/Redo must not reach the second Project');
  evidence.checks.push({ name: 'history-isolated', passed: true });

  await focusCanvas();
  await key('\uE007', true);
  assert.equal((await state()).editing, false, 'Ctrl+Enter is not Enter');
  await key('\uE007');
  await until(state, value => value.editing, 'Enter in second Project');
  await screenshot('second-editing');
  await switchWindow(first);
  await focusCanvas();
  await key('\uE00C');
  const firstAfter = await until(state, value => !value.editing, 'Escape in first Project');
  assert.equal(firstAfter.count, pasted.count);
  await screenshot('first-normal');
  await switchWindow(second);
  assert.equal((await state()).editing, true, 'Escape must not leave editing in the other Project');
  evidence.checks.push({ name: 'mode-and-escape-isolated', passed: true });
  evidence.windows = { firstBefore, secondBefore, firstAfter, secondAfter: await state() };
} catch (error) {
  evidence.error = error instanceof Error ? error.stack : String(error);
  console.error(evidence.error);
  if (session) {
    const screenshot = await request('GET', `/session/${session}/screenshot`).catch(() => null);
    if (screenshot) writeFileSync(path.join(output, 'failure.png'), Buffer.from(screenshot, 'base64'));
  }
} finally {
  const owned = roots.flatMap(root => processForestInstances([root]));
  if (session && request) await request('DELETE', `/session/${session}`).catch(() => {});
  for (const child of owned.reverse()) terminateProcessInstance(child);
  for (let attempt = 0; attempt < 100 && aliveProcessInstances(owned).length > 0; ++attempt) await delay(100);
  evidence.cleanupCompleted = aliveProcessInstances(owned).length === 0;
  evidence.sourceInputs.final = source();
  evidence.passed = !evidence.error && evidence.cleanupCompleted && evidence.checks.length === 3 &&
    JSON.stringify(evidence.sourceInputs.initial) === JSON.stringify(evidence.sourceInputs.final);
  writeFileSync(path.join(output, 'evidence.json'), JSON.stringify(evidence, null, 2) + '\n');
  console.log(`${evidence.passed ? 'PASS' : 'FAIL'}: ${path.join(output, 'evidence.json')}`);
  if (!evidence.passed) process.exitCode = 1;
}
