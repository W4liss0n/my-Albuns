import { spawn, execFileSync } from 'node:child_process';
import { mkdirSync, openSync, closeSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { aliveProcessInstances, waitForProcessInstance, processForestInstances, terminateProcessInstance } from './DevLifecycleProcessInstances.mjs';
import { createWebDriverClient, findFreeTcpPort, waitForHttp } from './GateWebDriver.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.resolve(process.argv[2] ?? path.join(root, '.scratch/frame-gesture-evidence', new Date().toISOString().replaceAll(':', '-')));
mkdirSync(output, { recursive: true });
const edge = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(root, 'scripts/Resolve-EdgeWebDriver.ps1')], { encoding: 'utf8', windowsHide: true }));
const roots = [];
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const source = () => ({
  gitCommit: execFileSync('git.exe', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim(),
  dirty: execFileSync('git.exe', ['status', '--porcelain'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim() !== '',
});
const evidence = {
  schemaVersion: 1, gate: 'frame-gesture-rendering', collectedAtUtc: new Date().toISOString(),
  browser: { version: edge.edgeVersion, mode: 'headless-new' },
  sourceInputs: { initial: source(), final: null }, passed: false, cleanupCompleted: false,
  scenarios: [],
};
async function start(executable, args, name) {
  const fd = openSync(path.join(output, name + '.log'), 'w');
  const child = spawn(executable, args, { cwd: root, windowsHide: true, stdio: ['ignore', fd, fd] }); closeSync(fd);
  try { roots.push(await waitForProcessInstance(child.pid, name)); }
  catch (error) { child.kill(); throw error; }
}
let request, session;
try {
  const vitePort = await findFreeTcpPort(); const driverPort = await findFreeTcpPort();
  await start(process.execPath, [path.join(root, 'node_modules/vite/bin/vite.js'), '--host', '127.0.0.1', '--port', String(vitePort), '--strictPort'], 'vite');
  await start(edge.driverExecutable, ['--port='+driverPort, '--host=127.0.0.1'], 'webdriver');
  await Promise.all([
    waitForHttp(`http://127.0.0.1:${vitePort}`, 'Vite'),
    waitForHttp(`http://127.0.0.1:${driverPort}/status`, 'Edge WebDriver'),
  ]);
  request = createWebDriverClient(`http://127.0.0.1:${driverPort}`);
  session = (await request('POST', '/session', { capabilities: { alwaysMatch: { browserName: 'MicrosoftEdge', 'ms:edgeOptions': { binary: edge.edgeExecutable, args: ['--headless=new', '--disable-gpu', '--no-first-run', '--window-size=1000,800'] } } } })).sessionId;
  const execute = (script, args=[]) => request('POST', `/session/${session}/execute/sync`, { script, args });
  const actions = list => request('POST', `/session/${session}/actions`, { actions: [{ type: 'pointer', id: 'mouse', parameters: { pointerType: 'mouse' }, actions: list }] });
  for (const action of ['resize', 'move']) {
    await request('POST', `/session/${session}/url`, { url: `http://127.0.0.1:${vitePort}/frame-gesture-preview.html?${action}` });
    let ready = false;
    for (let attempt = 0; attempt < 600; ++attempt) {
      ready = await execute("return document.body.dataset.ready === 'true'");
      if (ready) break;
      await delay(100);
    }
    if (!ready) throw new Error('The Frame gesture fixture did not become ready.');
    const point = await execute('return window.frameGestureTest.point(arguments[0])', [action]);
    const x = Math.round(point.x), y = Math.round(point.y);
    await actions([{ type: 'pointerMove', origin: 'viewport', x, y, duration: 0 }]);
    const initial = await execute('return window.frameGestureTest.sample()');
    await execute('window.frameGestureTest.start()');
    await actions([{ type: 'pointerDown', button: 0 }]);
    const samples = [];
    for (let i = 1; i <= 3; ++i) {
      const px = action === 'resize' ? x + i * 25 : x;
      const py = action === 'move' ? y - i * 25 : y;
      await actions([{ type: 'pointerMove', origin: 'viewport', x: px, y: py, duration: 0 }]);
      samples.push(await execute('return window.frameGestureTest.sample()'));
      await delay(100);
      await actions([{ type: 'pointerMove', origin: 'viewport', x: px, y: py, duration: 0 }]);
      samples.push(await execute('return window.frameGestureTest.sample()'));
    }
    const before = await execute('return window.frameGestureTest.sample()');
    writeFileSync(path.join(output, `${action}-before-release.png`), Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'));
    await actions([{ type: 'pointerUp', button: 0 }]);
    let completion;
    for (let attempt = 0; attempt < 100; ++attempt) {
      completion = await execute(`
        const completion = window.frameGestureTest.completion();
        return { ...completion, frameAfterPresentation: completion.presentedAt !== null &&
          window.frameGestureTest.trace().some(frame => frame.time > completion.presentedAt) };
      `);
      if (completion.committedAt !== null && completion.frameAfterPresentation) break;
      await delay(50);
    }
    const trace = await execute('return window.frameGestureTest.trace()');
    writeFileSync(path.join(output, `${action}-after-release.png`), Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'));
    const expected = action === 'resize' ? 'ew-resize' : 'move';
    const cursorFailure = samples.some(item => item.cursor !== expected);
    const afterRelease = trace.filter(item => item.time >= before.time);
    const frameFlash = afterRelease
      .some(item => item.x !== before.x || item.y !== before.y || item.width !== before.width);
    const exercised = completion.committedAt !== null && completion.frameAfterPresentation &&
      afterRelease.length >= 2 && (
      action === 'resize' ? before.width !== initial.width : before.y !== initial.y
    );
    await actions([{ type: 'pointerMove', origin: 'viewport', x: 5, y: 5, duration: 0 }]);
    const cursorAfterRelease = (await execute('return window.frameGestureTest.sample()')).cursor;
    const cursorReleased = cursorAfterRelease === 'auto' || cursorAfterRelease === 'default';
    const passed = exercised && cursorReleased && !cursorFailure && !frameFlash;
    evidence.scenarios.push({ action, passed, exercised, expectedCursor: expected,
      cursorFailure, cursorAfterRelease, frameFlash, completion, samples, before, trace });
    console.log(action + ' cursors: ' + samples.map(item => item.cursor).join(' -> '));
    console.log(action + ' frame flash: ' + frameFlash + '; cursor released: ' + cursorReleased);
  }
} catch (error) {
  evidence.error = error instanceof Error ? error.stack : String(error);
  console.error(evidence.error);
} finally {
  if (session && request) await request('DELETE', `/session/${session}`).catch(() => {});
  const owned = roots.flatMap(root => processForestInstances([root]));
  for (const child of owned.reverse()) terminateProcessInstance(child);
  for (let attempt = 0; attempt < 100 && aliveProcessInstances(owned).length > 0; ++attempt) await delay(100);
  evidence.cleanupCompleted = aliveProcessInstances(owned).length === 0;
  evidence.sourceInputs.final = source();
  evidence.passed = !evidence.error && evidence.cleanupCompleted &&
    evidence.scenarios.length === 2 && evidence.scenarios.every(item => item.passed) &&
    JSON.stringify(evidence.sourceInputs.initial) === JSON.stringify(evidence.sourceInputs.final);
  writeFileSync(path.join(output, 'evidence.json'), JSON.stringify(evidence, null, 2) + '\n');
  console.log('Frame gesture rendering evidence: ' + path.join(output, 'evidence.json'));
  if (!evidence.passed) process.exitCode = 1;
}
