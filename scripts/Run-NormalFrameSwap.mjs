import { spawn, execFileSync } from 'node:child_process';
import { mkdirSync, openSync, closeSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import { aliveProcessInstances, waitForProcessInstance, processForestInstances, terminateProcessInstance } from './DevLifecycleProcessInstances.mjs';
import { createWebDriverClient, findFreeTcpPort, waitForHttp } from './GateWebDriver.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.resolve(process.argv[2] ?? path.join(root, '.scratch/normal-frame-swap-gestures'));
const source = () => ({
  gitCommit: execFileSync('git.exe', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim(),
  dirty: execFileSync('git.exe', ['status', '--porcelain'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim() !== '',
});
const evidence = { schemaVersion: 1, gate: 'normal-frame-swap-gestures', collectedAtUtc: new Date().toISOString(),
  sourceInputs: { initial: source(), final: null }, passed: false, cleanupCompleted: false, scenarios: [] };
mkdirSync(output, { recursive: true });
const edge = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(root, 'scripts/Resolve-EdgeWebDriver.ps1')], { encoding: 'utf8', windowsHide: true }));
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
  request = createWebDriverClient(`http://127.0.0.1:${driverPort}`, { defaultTimeoutMilliseconds: 60000 }); console.log("starting browser");
  session = (await request('POST', '/session', { capabilities: { alwaysMatch: { browserName: 'MicrosoftEdge', 'ms:edgeOptions': { binary: edge.edgeExecutable, args: ['--headless=new', '--disable-gpu', '--no-first-run', '--window-size=1024,859'] } } } })).sessionId;
  console.log("browser ready"); const execute = (script, args = []) => request('POST', `/session/${session}/execute/sync`, { script, args });
  const key = (type, value) => request('POST', `/session/${session}/actions`, { actions: [{ type: 'key', id: 'keyboard', actions: [{ type, value }] }] });
  const pointer = actions => request('POST', `/session/${session}/actions`, {actions:[{type:'pointer',id:'mouse',parameters:{pointerType:'mouse'},actions}]});
  const move = point => ({type:'pointerMove',origin:'viewport',...point,duration:120});
  const point = () => execute("return window.normalSwapTest.point('swap-frame-0')");
  const state = () => execute("return {photos:document.body.dataset.frameSwapAllPhotos,selection:document.body.dataset.frameSwapSelection,intent:document.body.dataset.frameSwapLastIntent,editing:document.querySelector('.canvas-host canvas')?.getAttribute('aria-label')?.startsWith('Canvas da Lâmina em edição')} ");
  for (const name of ['click','double-click','alt-pan','cross-sheet-scroll','escape']) {
    await request('POST', `/session/${session}/url`, {url:`http://127.0.0.1:${port}/workspace-preview.html?frame=swap&swap=cross-photos&mode=normal`});
    let ready = false;
    for(let i=0;i<600;i++) {
      ready=await execute('return Boolean(document.querySelector(".canvas-host canvas"))');
      if(ready)break;
      await delay(100);
    }
    assert.ok(ready,'Canvas must render'); await delay(300);
    const initial=await state(), source=await point();
    if(name==='click'||name==='double-click') {
      const click=[move(source),{type:'pointerDown',button:0},{type:'pointerUp',button:0}];
      await pointer(name==='double-click' ? [...click,{type:'pause',duration:60},{type:'pointerDown',button:0},{type:'pointerUp',button:0}] : click);
      await delay(200);
      if(name==='click')assert.equal((await state()).selection,'swap-frame-0');
      else assert.equal((await state()).editing,true);
      assert.equal((await state()).photos,initial.photos);
    } else if(name==='alt-pan') {
      await key('keyDown','\uE00A');
      await pointer([move(source),{type:'pointerDown',button:0},move({x:source.x+30,y:source.y+15}),{type:'pointerUp',button:0}]);
      await key('keyUp','\uE00A'); await delay(200);
      const intent=JSON.parse((await state()).intent);
      assert.equal(intent.kind,'transformPhoto'); assert.equal(intent.frameId,'swap-frame-0');
      assert.notEqual(intent.deltaPanX,0); assert.equal((await state()).photos,initial.photos);
    } else {
      const bounds=await execute('return document.querySelector(".canvas-host canvas").getBoundingClientRect().toJSON()');
      assert.ok((await execute("return window.normalSwapTest.point('swap-frame-4')")).x>bounds.right,'destination starts outside viewport');
      const begin=[move(source),{type:'pointerDown',button:0},move({x:Math.floor(bounds.right-5),y:source.y})];
      if(name==='escape') {
        await pointer(begin);
        await key('keyDown','\uE00C'); await key('keyUp','\uE00C');
        await pointer([{type:'pointerUp',button:0}]); await delay(200);
        assert.equal((await state()).photos,initial.photos); assert.equal((await state()).selection,'');
      } else {
        const target=await execute(`const point=window.normalSwapTest.point('swap-frame-4');
          const scrollbar=document.querySelector('[role="scrollbar"][aria-label="Navegação horizontal das Lâminas"]');
          return {x:Math.round(point.x-Number(scrollbar.getAttribute('aria-valuemax'))+Number(scrollbar.getAttribute('aria-valuenow'))),y:point.y};`);
        // One W3C sequence retains native button/capture state across the edge pause.
        await pointer([...begin,{type:'pause',duration:1800},move(target),{type:'pause',duration:100},{type:'pointerUp',button:0}]);
        await delay(300);
        const revealed=await execute("return window.normalSwapTest.point('swap-frame-4')");
        assert.ok(revealed.x>bounds.left&&revealed.x<bounds.right-170,'auto-scroll must reveal the destination');
        const after=await state();
        assert.equal(after.photos,'00000000-0000-4000-8000-000000000002,00000000-0000-4000-8000-000000000002,empty,empty,00000000-0000-4000-8000-000000000001,empty');
        assert.equal(after.selection,'');
      }
    }
    writeFileSync(path.join(output,name+'.png'),Buffer.from(await request('GET',`/session/${session}/screenshot`),'base64'));
    evidence.scenarios.push({name,passed:true}); console.log('PASS',name);
  }
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
  evidence.passed = !evidence.error && evidence.cleanupCompleted && evidence.scenarios.length === 5 &&
    evidence.scenarios.every(item => item.passed) && JSON.stringify(evidence.sourceInputs.initial) === JSON.stringify(evidence.sourceInputs.final);
  writeFileSync(path.join(output, 'evidence.json'), JSON.stringify(evidence, null, 2) + '\n');
  console.log('Normal Frame swap evidence: ' + path.join(output, 'evidence.json'));
  if (!evidence.passed) process.exitCode = 1;
}
