import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import { createHeadlessBrowserSession } from './HeadlessBrowserSession.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.resolve(process.argv[2] ?? path.join(root, '.scratch/normal-frame-swap-gestures'));
const source = () => ({
  gitCommit: execFileSync('git.exe', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim(),
  dirty: execFileSync('git.exe', ['status', '--porcelain'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim() !== '',
});
const evidence = { schemaVersion: 1, gate: 'normal-frame-swap-gestures', collectedAtUtc: new Date().toISOString(),
  sourceInputs: { initial: source(), final: null }, passed: false, cleanupCompleted: false, scenarios: [] };
mkdirSync(output, { recursive: true });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const browser = createHeadlessBrowserSession({ root, output, windowSize: '1024,859', requestTimeoutMilliseconds: 120000 });
let request, session;
try {
  const started = await browser.start();
  ({ request, session } = started);
  const port = started.port;

  console.log("browser ready"); const execute = (script, args = []) => request('POST', `/session/${session}/execute/sync`, { script, args });
  const key = (type, value) => request('POST', `/session/${session}/actions`, { actions: [{ type: 'key', id: 'keyboard', actions: [{ type, value }] }] });
  const pointer = actions => request('POST', `/session/${session}/actions`, {actions:[{type:'pointer',id:'mouse',parameters:{pointerType:'mouse'},actions}]});
  const move = point => ({type:'pointerMove',origin:'viewport',...point,duration:120});
  const point = () => execute("return window.normalSwapTest.point('swap-frame-0')");
  // Waits for the gesture's own outcome instead of a fixed pause, so a slow
  // runner neither fails early nor passes before the page reacted.
  const until = async (description, read, accept) => {
    const deadline = Date.now() + 10000;
    for (;;) {
      const value = await read();
      if (accept(value)) return value;
      assert.ok(Date.now() < deadline, description);
      await delay(25);
    }
  };
  const gestureEnded = () => until('the Frame gesture must end',
    () => execute('return document.querySelector(".canvas-host canvas").classList.contains("pixi-canvas--frame-gesture")'), active => !active);
  const state = () => execute("return {photos:document.body.dataset.frameSwapAllPhotos,selection:document.body.dataset.frameSwapSelection,intent:document.body.dataset.frameSwapLastIntent,editing:document.querySelector('.canvas-host canvas')?.getAttribute('aria-label')?.startsWith('área de edição da lâmina em edição')} ");
  for (const name of ['click','double-click','alt-pan','cross-sheet-scroll','escape','feedback']) {
    await request('POST', `/session/${session}/url`, {url:`http://127.0.0.1:${port}/previews/workspace-preview.html?frame=swap&swap=cross-photos&mode=normal`});
    let ready = false;
    for(let i=0;i<600;i++) {
      ready=await execute('return Boolean(document.querySelector(".canvas-host canvas"))');
      if(ready)break;
      await delay(100);
    }
    assert.ok(ready,'Canvas must render');
    const initial=await until('the swap fixture must expose its Photos', state, current => Boolean(current.photos)), source=await point();
    if(name==='click'||name==='double-click') {
      const click=[move(source),{type:'pointerDown',button:0},{type:'pointerUp',button:0}];
      await pointer(name==='double-click' ? [...click,{type:'pause',duration:60},{type:'pointerDown',button:0},{type:'pointerUp',button:0}] : click);
      if(name==='click')await until('a click must select the Frame', state, current => current.selection==='swap-frame-0');
      else await until('a double click must enter Sheet editing', state, current => current.editing===true);
      assert.equal((await state()).photos,initial.photos);
    } else if(name==='alt-pan') {
      await key('keyDown','\uE00A');
      await pointer([move(source),{type:'pointerDown',button:0},move({x:source.x+30,y:source.y+15}),{type:'pointerUp',button:0}]);
      await key('keyUp','\uE00A');
      const intent=JSON.parse((await until('Alt drag must commit a Photo transform', state, current => Boolean(current.intent))).intent);
      assert.equal(intent.kind,'transformPhoto'); assert.equal(intent.frameId,'swap-frame-0');
      assert.notEqual(intent.deltaPanX,0); assert.equal((await state()).photos,initial.photos);
    } else if(name==='feedback') {
      const target = await execute("return window.normalSwapTest.point('swap-frame-1')");
      const bounds = await execute('return document.querySelector(".canvas-host canvas").getBoundingClientRect().toJSON()');
      await execute(`window.dragFeedbackSamples=[];
        window.dragFeedbackTimer=setInterval(()=>{
          const canvas=document.querySelector('.canvas-host canvas');
          if(canvas.classList.contains('pixi-canvas--frame-gesture'))window.dragFeedbackSamples.push({
            cursor:getComputedStyle(canvas).cursor,ghost:canvas.dataset.frameContentDragGhost,target:canvas.dataset.frameContentDragTarget});
        },16);`);
      await pointer([move(source),{type:'pointerDown',button:0},move(target),{type:'pause',duration:200},
        move({x:target.x+8,y:target.y+8}),{type:'pause',duration:100},
        move({x:Math.round(bounds.left+60),y:Math.round(bounds.top+70)}),{type:'pause',duration:200},{type:'pointerUp',button:0}]);
      const samples = await execute('clearInterval(window.dragFeedbackTimer); return window.dragFeedbackSamples');
      assert.ok(samples.length>=5,'feedback sampled during the real gesture');
      assert.ok(samples.every(sample=>sample.cursor==='grabbing'),'cursor stays grabbing across targets and empty areas');
      assert.ok(samples.every(sample=>sample.ghost==='swap-frame-0'),'ghost persists throughout the drag');
      assert.ok(samples.some(sample=>sample.target==='swap-frame-1'),'valid destination is visibly highlighted');
      assert.ok(samples.some(sample=>sample.target===''),'empty areas have no destination highlight');
      const after=await execute('const canvas=document.querySelector(".canvas-host canvas"); return {ghost:canvas.dataset.frameContentDragGhost,target:canvas.dataset.frameContentDragTarget,cursor:getComputedStyle(canvas).cursor}');
      assert.equal(after.ghost??null,null); assert.equal(after.target??null,null); assert.notEqual(after.cursor,'grabbing');
      assert.equal((await state()).photos,initial.photos);
    } else {
      const bounds=await execute('return document.querySelector(".canvas-host canvas").getBoundingClientRect().toJSON()');
      assert.ok((await execute("return window.normalSwapTest.point('swap-frame-4')")).x>bounds.right,'destination starts outside viewport');
      const begin=[move(source),{type:'pointerDown',button:0},move({x:Math.floor(bounds.right-5),y:source.y})];
      if(name==='escape') {
        await pointer(begin);
        await key('keyDown','\uE00C'); await key('keyUp','\uE00C');
        await pointer([{type:'pointerUp',button:0}]); await gestureEnded();
        assert.equal((await state()).photos,initial.photos); assert.equal((await state()).selection,'');
      } else {
        const target=await execute(`const point=window.normalSwapTest.point('swap-frame-4');
          const scrollbar=document.querySelector('[role="scrollbar"][aria-label="Navegação horizontal das lâminas"]');
          return {x:Math.round(point.x-Number(scrollbar.getAttribute('aria-valuemax'))+Number(scrollbar.getAttribute('aria-valuenow'))),y:point.y};`);
        // One W3C sequence retains native button/capture state across the edge pause.
        await pointer([...begin,{type:'pause',duration:1800},move(target),{type:'pause',duration:100},{type:'pointerUp',button:0}]);
        const after=await until('the cross-Sheet drop must swap the Photos', state,
          current => current.photos==='00000000-0000-4000-8000-000000000002,00000000-0000-4000-8000-000000000002,empty,empty,00000000-0000-4000-8000-000000000001,empty');
        const revealed=await execute("return window.normalSwapTest.point('swap-frame-4')");
        assert.ok(revealed.x>bounds.left&&revealed.x<bounds.right-170,'auto-scroll must reveal the destination');
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
  evidence.cleanupCompleted = await browser.close();
  evidence.sourceInputs.final = source();
  evidence.passed = !evidence.error && evidence.cleanupCompleted && evidence.scenarios.length === 6 &&
    evidence.scenarios.every(item => item.passed) && JSON.stringify(evidence.sourceInputs.initial) === JSON.stringify(evidence.sourceInputs.final);
  writeFileSync(path.join(output, 'evidence.json'), JSON.stringify(evidence, null, 2) + '\n');
  console.log('Normal Frame swap evidence: ' + path.join(output, 'evidence.json'));
  if (!evidence.passed) process.exitCode = 1;
}
