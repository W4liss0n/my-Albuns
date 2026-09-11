import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { mkdirSync, openSync, closeSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { aliveProcessInstances, waitForProcessInstance, processForestInstances, terminateProcessInstance } from './DevLifecycleProcessInstances.mjs';
import { createWebDriverClient, findFreeTcpPort, waitForHttp } from './GateWebDriver.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.resolve(process.argv[2] ?? path.join(root, '.scratch/ui-acceptance/photo-placement'));
mkdirSync(output, { recursive: true });
const edge = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(root, 'scripts/Resolve-EdgeWebDriver.ps1')], { encoding: 'utf8', windowsHide: true }));
const roots = [];
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const evidence = { gate: 'photo-placement-rendering', collectedAtUtc: new Date().toISOString(),
  browserVersion: edge.edgeVersion, passed: false, cleanupCompleted: false, samples: [] };
async function start(executable, args, name) {
  const fd = openSync(path.join(output, name + '.log'), 'w');
  const child = spawn(executable, args, { cwd: root, windowsHide: true, stdio: ['ignore', fd, fd] }); closeSync(fd);
  try { roots.push(await waitForProcessInstance(child.pid, name)); }
  catch (error) { child.kill(); throw error; }
}
let request, session;
try {
  const port = await findFreeTcpPort(), driverPort = await findFreeTcpPort();
  await start(process.execPath, [path.join(root, 'node_modules/vite/bin/vite.js'), '--host', '127.0.0.1', '--port', String(port), '--strictPort'], 'vite');
  await start(edge.driverExecutable, ['--port=' + driverPort, '--host=127.0.0.1'], 'webdriver');
  await Promise.all([waitForHttp(`http://127.0.0.1:${port}`, 'Vite'), waitForHttp(`http://127.0.0.1:${driverPort}/status`, 'WebDriver')]);
  request = createWebDriverClient(`http://127.0.0.1:${driverPort}`);
  session = (await request('POST', '/session', { capabilities: { alwaysMatch: {
    browserName: 'MicrosoftEdge', 'ms:edgeOptions': { binary: edge.edgeExecutable,
      args: ['--headless=new', '--disable-gpu', '--no-first-run', '--window-size=1100,800'] },
  } } })).sessionId;
  await request('POST', `/session/${session}/url`, { url: `http://127.0.0.1:${port}/photo-placement-preview.html` });
  const execute = script => request('POST', `/session/${session}/execute/sync`, { script, args: [] });
  let ready = false;
  for (let i = 0; i < 200 && !ready; i++) {
    ready = await execute("return document.body.dataset.ready === 'true'");
    if (!ready) await delay(100);
  }
  assert.ok(ready, 'Photo placement fixture must finish');
  evidence.samples = await execute('return window.photoPlacementTest.samples');
  evidence.svgSample = await execute('return window.photoPlacementTest.svgSample');
  evidence.openingSample = await execute('return window.photoPlacementTest.openingSample');
  writeFileSync(path.join(output, 'rendered.png'), Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'));
  assert.equal(evidence.samples.length, 12);
  for (const sample of evidence.samples) {
    assert.deepEqual(sample.first, sample.expected, `Photo ${sample.index + 1} must have image pixels in its first render`);
    assert.deepEqual(sample.settled, sample.first, `Photo ${sample.index + 1} must not change from fallback to image`);
  }
  assert.deepEqual(evidence.svgSample.actual, evidence.svgSample.expected, 'SVG previews must retain rasterized image pixels');
  assert.deepEqual(evidence.openingSample.beforeUrl, [0, 0, 0, 0], 'Opening must not show synthetic artwork before the preview URL arrives');
  assert.deepEqual(evidence.openingSample.beforeTexture, [0, 0, 0, 0], 'Opening must not show synthetic artwork while the texture loads');
  assert.deepEqual(evidence.openingSample.ready, evidence.openingSample.expected, 'Opening must display the real preview when it loads');
  console.log('PASS: 12 additions show the loaded photo on the first render, with identical settled pixels.');
  console.log('PASS: project opening stays free of synthetic photo pixels until the real preview loads.');
} catch (error) {
  evidence.error = error instanceof Error ? error.stack : String(error);
  console.error(evidence.error);
} finally {
  const owned = roots.flatMap(root => processForestInstances([root]));
  if (session && request) await request('DELETE', `/session/${session}`).catch(() => {});
  for (const instance of owned.reverse()) terminateProcessInstance(instance);
  for (let i = 0; i < 100 && aliveProcessInstances(owned).length; i++) await delay(100);
  evidence.cleanupCompleted = aliveProcessInstances(owned).length === 0;
  evidence.passed = !evidence.error && evidence.cleanupCompleted;
  writeFileSync(path.join(output, 'evidence.json'), JSON.stringify(evidence, null, 2) + '\n');
  if (!evidence.passed) process.exitCode = 1;
}
