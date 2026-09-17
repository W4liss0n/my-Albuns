import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHeadlessBrowserSession } from './HeadlessBrowserSession.mjs';
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.resolve(process.argv[2] ?? path.join(root, '.scratch/frame-gesture-evidence', new Date().toISOString().replaceAll(':', '-')));
mkdirSync(output, { recursive: true });
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const source = () => ({
  gitCommit: execFileSync('git.exe', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim(),
  dirty: execFileSync('git.exe', ['status', '--porcelain'], { cwd: root, encoding: 'utf8', windowsHide: true }).trim() !== '',
});
const evidence = {
  schemaVersion: 1, gate: 'frame-gesture-rendering', collectedAtUtc: new Date().toISOString(),
  sourceInputs: { initial: source(), final: null }, passed: false, cleanupCompleted: false,
  scenarios: [],
};
const browser = createHeadlessBrowserSession({ root, output, windowSize: '1000,800', requestTimeoutMilliseconds: 10000 });
let request, session;
try {
  const started = await browser.start();
  ({ request, session } = started);
  const port = started.port;
  evidence.browser = { version: started.edge.edgeVersion, mode: 'headless-new' };

  const execute = (script, args=[]) => request('POST', `/session/${session}/execute/sync`, { script, args });
  const actions = list => request('POST', `/session/${session}/actions`, { actions: [{ type: 'pointer', id: 'mouse', parameters: { pointerType: 'mouse' }, actions: list }] });
  const key = (type, value) => request('POST', `/session/${session}/actions`, {
    actions: [{ type: 'key', id: 'keyboard', actions: [{ type, value }] }],
  });
  for (const scenario of ['resize', 'move', 'group-resize', 'group-move']) {
    const action = scenario.endsWith('resize') ? 'resize' : 'move';
    const group = scenario.startsWith('group-');
    await request('POST', `/session/${session}/url`, { url: `http://127.0.0.1:${port}/frame-gesture-preview.html?${scenario}` });
    let ready = false;
    for (let attempt = 0; attempt < 600; ++attempt) {
      ready = await execute("return document.body.dataset.ready === 'true'");
      if (ready) break;
      await delay(100);
    }
    if (!ready) throw new Error('The Frame gesture fixture did not become ready.');
    const selectionChecks = [];
    if (group) {
      const click = async (target, control = false) => {
        const point = await execute('return window.frameGestureTest.point(arguments[0])', [target]);
        if (control) await key('keyDown', '\uE009');
        await actions([{ type: 'pointerMove', origin: 'viewport', x: Math.round(point.x), y: Math.round(point.y), duration: 0 },
          { type: 'pointerDown', button: 0 }, { type: 'pointerUp', button: 0 }]);
        if (control) await key('keyUp', '\uE009');
        return execute('return window.frameGestureTest.selection()');
      };
      for (const [target, control, expected] of [
        ['overlap', false, ['frame-002']],
        ['move', true, ['frame-002', 'frame-001']],
        ['overlap', true, ['frame-001']],
        ['overlap', true, ['frame-001', 'frame-002']],
        ['move', false, ['frame-001']],
        ['empty', false, []],
        ['move', false, ['frame-001']],
        ['overlap', true, ['frame-001', 'frame-002']],
      ]) {
        const actual = await click(target, control);
        selectionChecks.push({ target, control, expected, actual, passed: JSON.stringify(actual) === JSON.stringify(expected) });
      }
    }
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
    writeFileSync(path.join(output, `${scenario}-before-release.png`), Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'));
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
    writeFileSync(path.join(output, `${scenario}-after-release.png`), Buffer.from(await request('GET', `/session/${session}/screenshot`), 'base64'));
    const expected = action === 'resize' ? 'ew-resize' : 'move';
    const cursorFailure = samples.some(item => item.cursor !== expected);
    const afterRelease = trace.filter(item => item.time >= before.time);
    const frameFlash = afterRelease
      .some(item => item.x !== before.x || item.y !== before.y || item.width !== before.width || JSON.stringify(item.members) !== JSON.stringify(before.members));
    const exercised = completion.committedAt !== null && completion.frameAfterPresentation &&
      afterRelease.length >= 2 && (
      action === 'resize' ? before.width !== initial.width : before.y !== initial.y
    );
    await actions([{ type: 'pointerMove', origin: 'viewport', x: 5, y: 5, duration: 0 }]);
    const cursorAfterRelease = (await execute('return window.frameGestureTest.sample()')).cursor;
    const cursorReleased = cursorAfterRelease === 'auto' || cursorAfterRelease === 'default';
    const membersExercised = before.members.length === (group ? 2 : 1) && before.members.every((member, index) =>
      action === 'resize' ? member.width !== initial.members[index].width : member.y !== initial.members[index].y);
    const selectionRetained = !group || JSON.stringify(await execute('return window.frameGestureTest.selection()')) === JSON.stringify(['frame-001', 'frame-002']);
    const passed = exercised && membersExercised && selectionRetained && selectionChecks.every(check => check.passed) && cursorReleased && !cursorFailure && !frameFlash;
    evidence.scenarios.push({ action: scenario, passed, exercised, membersExercised, selectionRetained, selectionChecks, expectedCursor: expected,
      cursorFailure, cursorAfterRelease, frameFlash, completion, samples, before, trace });
    console.log(scenario + ' cursors: ' + samples.map(item => item.cursor).join(' -> '));
    console.log(scenario + ' frame flash: ' + frameFlash + '; cursor released: ' + cursorReleased);
  }
} catch (error) {
  evidence.error = error instanceof Error ? error.stack : String(error);
  console.error(evidence.error);
} finally {
  evidence.cleanupCompleted = await browser.close();
  evidence.sourceInputs.final = source();
  evidence.passed = !evidence.error && evidence.cleanupCompleted &&
    evidence.scenarios.length === 4 && evidence.scenarios.every(item => item.passed) &&
    JSON.stringify(evidence.sourceInputs.initial) === JSON.stringify(evidence.sourceInputs.final);
  writeFileSync(path.join(output, 'evidence.json'), JSON.stringify(evidence, null, 2) + '\n');
  console.log('Frame gesture rendering evidence: ' + path.join(output, 'evidence.json'));
  if (!evidence.passed) process.exitCode = 1;
}
