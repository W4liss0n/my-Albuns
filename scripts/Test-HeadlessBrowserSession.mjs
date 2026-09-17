import assert from 'node:assert/strict';
import test from 'node:test';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createHeadlessBrowserSession } from './HeadlessBrowserSession.mjs';
import { aliveProcessInstances, waitForProcessInstance, terminateProcessInstance } from './DevLifecycleProcessInstances.mjs';

test('partial acquisition releases its Vite process and preserves an unrelated process', { skip: process.platform !== 'win32' }, async () => {
  const root = mkdtempSync(path.join(tmpdir(), 'myalbuns-headless-'));
  const output = path.join(root, 'output');
  mkdirSync(path.join(root, 'scripts'), { recursive: true });
  mkdirSync(path.join(root, 'node_modules/vite/bin'), { recursive: true });
  mkdirSync(output);
  const marker = path.join(root, 'started.json');
  writeFileSync(path.join(root, 'node_modules/vite/bin/vite.js'),
    `require('node:fs').writeFileSync(${JSON.stringify(marker)}, JSON.stringify({ pid: process.pid })); setInterval(() => {}, 1000);`);
  const edge = { driverExecutable: path.join(root, 'missing-driver.exe'), edgeExecutable: process.execPath };
  writeFileSync(path.join(root, 'scripts/Resolve-EdgeWebDriver.ps1'),
    `[Console]::Out.Write('${JSON.stringify(edge).replaceAll("'", "''")}')`);
  const unrelated = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], { windowsHide: true, stdio: 'ignore' });
  await once(unrelated, 'spawn');
  const unrelatedInstance = await waitForProcessInstance(unrelated.pid, 'unrelated fixture');
  const browser = createHeadlessBrowserSession({ root, output, windowSize: '1000,800' });
  try {
    await assert.rejects(browser.start(), /ENOENT/);
    const vite = await waitForProcessInstance(JSON.parse(readFileSync(marker, 'utf8')).pid, 'fixture Vite');
    assert.equal(await browser.close(), true);
    assert.equal(await browser.close(), true);
    assert.equal(aliveProcessInstances([vite]).length, 0);
    assert.equal(aliveProcessInstances([unrelatedInstance]).length, 1);
  } finally {
    await browser.close();
    terminateProcessInstance(unrelatedInstance);
    assert.equal(path.dirname(path.resolve(root)), path.resolve(tmpdir()));
    assert.match(path.basename(root), /^myalbuns-headless-/);
    rmSync(root, { recursive: true, force: true });
  }
});
