import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { openSync, closeSync } from 'node:fs';
import path from 'node:path';
import { aliveProcessInstances, waitForProcessInstance, processForestInstances, terminateProcessInstance } from './DevLifecycleProcessInstances.mjs';
import { createWebDriverClient, findFreeTcpPort, waitForHttp } from './GateWebDriver.mjs';

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));

/** Owns Vite, WebDriver and its headless descendants, including partial acquisition. */
export function createHeadlessBrowserSession({ root, output, windowSize, requestTimeoutMilliseconds = 10000 }) {
  const roots = [];
  let request, session;
  let cleanup;
  async function startProcess(executable, args, name) {
    const fd = openSync(path.join(output, `${name}.log`), 'w');
    let child;
    try {
      child = spawn(executable, args, { cwd: root, windowsHide: true, stdio: ['ignore', fd, fd] });
    } finally { closeSync(fd); }
    await once(child, 'spawn');
    try { roots.push(await waitForProcessInstance(child.pid, name)); }
    catch (error) { child.kill(); throw error; }
  }
  return {
    async start() {
      const edge = JSON.parse(execFileSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
        path.join(root, 'scripts/Resolve-EdgeWebDriver.ps1')], { encoding: 'utf8', windowsHide: true }));
      const port = await findFreeTcpPort(), driverPort = await findFreeTcpPort();
      await startProcess(process.execPath, [path.join(root, 'node_modules/vite/bin/vite.js'), '--host', '127.0.0.1', '--port', String(port), '--strictPort'], 'vite');
      await startProcess(edge.driverExecutable, [`--port=${driverPort}`, '--host=127.0.0.1'], 'webdriver');
      await Promise.all([waitForHttp(`http://127.0.0.1:${port}`, 'Vite'), waitForHttp(`http://127.0.0.1:${driverPort}/status`, 'Edge WebDriver')]);
      request = createWebDriverClient(`http://127.0.0.1:${driverPort}`, { defaultTimeoutMilliseconds: requestTimeoutMilliseconds });
      session = (await request('POST', '/session', { capabilities: { alwaysMatch: {
        browserName: 'MicrosoftEdge', 'ms:edgeOptions': { binary: edge.edgeExecutable,
          args: ['--headless=new', '--disable-gpu', '--no-first-run', `--window-size=${windowSize}`] },
      } } })).sessionId;
      return { port, request, session, edge };
    },
    close() {
      // Snapshot descendants before session deletion can detach their parent.
      cleanup ??= (async () => {
        const owned = roots.flatMap(instance => processForestInstances([instance]));
        if (session && request) await request('DELETE', `/session/${session}`).catch(() => {});
        for (const instance of owned.reverse()) terminateProcessInstance(instance);
        for (let attempt = 0; attempt < 100 && aliveProcessInstances(owned).length > 0; ++attempt) await delay(100);
        return aliveProcessInstances(owned).length === 0;
      })();
      return cleanup;
    },
  };
}
