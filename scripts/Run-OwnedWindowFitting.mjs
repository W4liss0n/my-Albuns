import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import path from "node:path";
import { createWebDriverClient, findFreeTcpPort, waitForHttp } from "./GateWebDriver.mjs";

const [edgeExecutable, driverExecutable, outputArgument] = process.argv.slice(2);
if (!edgeExecutable || !driverExecutable || !outputArgument) {
  throw new Error("Usage: Run-OwnedWindowFitting.mjs <edge> <driver> <output-directory>");
}
const output = path.resolve(outputArgument);
const dist = path.resolve("dist");
mkdirSync(output, { recursive: true });
const missing = {
  kind: "exportMediaProblems", projectName: "Projeto de teste", busy: false, message: "",
  problems: [{ mediaId: "photo-1", fileName: "Foto movida.jpg", state: "absent" }],
};
const presentation = { sessionId: "owned-window-fitting", state: missing };

// Only the OS boundary is substituted: production React, CSS, ResizeObserver,
// dialog events and the Tauri window adapter run inside the real browser.
function installNativeBoundary() {
  let callbackId = 0;
  const callbacks = new Map();
  const listeners = new Map();
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: (_event, id) => listeners.delete(id),
  };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "project-dialog" } },
    transformCallback: (callback) => { callbacks.set(++callbackId, callback); return callbackId; },
    unregisterCallback: (id) => callbacks.delete(id),
    invoke: async (command, args) => {
      if (command === "plugin:window|set_size") {
        const size = JSON.parse(JSON.stringify(args.value)).Logical;
        if (!size || !Number.isFinite(size.height)) throw new Error("Invalid native size request");
        parent.fitting.fits.push(size.height);
        parent.fitting.lastFit = Date.now();
        parent.document.querySelector("iframe").style.height = `${size.height}px`;
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        return;
      }
      if (command === "plugin:window|center") return;
      if (command === "owned_window_content_ready") { parent.fitting.ready++; return; }
      if (command === "current_project_dialog_presentation") return parent.fitting.presentation;
      if (command === "plugin:event|listen") { listeners.set(args.handler, args.event); return args.handler; }
      if (command === "plugin:event|unlisten") { listeners.delete(args.eventId); return; }
      throw new Error(`Unexpected native command: ${command}`);
    },
  };
  window.presentFittingState = (next) => {
    for (const [id, event] of listeners) {
      if (event === "myalbuns://project-dialog-presentation") callbacks.get(id)?.({ event, id, payload: next });
    }
  };
  addEventListener("error", event => parent.fitting.errors.push(event.message));
  addEventListener("unhandledrejection", event => parent.fitting.errors.push(String(event.reason)));
}

const server = createServer((request, response) => {
  const url = new URL(request.url, "http://localhost");
  if (url.pathname === "/") {
    response.setHeader("Content-Type", "text/html; charset=utf-8");
    response.end(`<html><body><script>window.fitting={fits:[],errors:[],lastFit:0,ready:0,presentation:${JSON.stringify(presentation)}};</script><iframe style="width:640px;height:438px;border:0" src="/project-dialog.html?presentation=${encodeURIComponent(JSON.stringify(presentation))}&ownedReadyToken=1"></iframe></body></html>`);
    return;
  }
  const file = path.resolve(dist, `.${decodeURIComponent(url.pathname)}`);
  if (!file.startsWith(`${dist}${path.sep}`)) { response.writeHead(403).end(); return; }
  try {
    let bytes = readFileSync(file);
    const extension = path.extname(file);
    response.setHeader("Content-Type", ({ ".html": "text/html; charset=utf-8", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml" })[extension] ?? "application/octet-stream");
    if (extension === ".html") bytes = Buffer.from(bytes.toString().replace("<head>", `<head><script>(${installNativeBoundary.toString()})();</script>`));
    response.end(bytes);
  } catch { response.writeHead(404).end(); }
});
await new Promise(resolve => server.listen(0, "127.0.0.1", resolve));
const port = await findFreeTcpPort();
const child = spawn(driverExecutable, [`--port=${port}`, "--host=127.0.0.1"], { windowsHide: true, stdio: "ignore" });
let request, sessionId;
const results = [];
try {
  await waitForHttp(`http://127.0.0.1:${port}/status`, "owned window fitting driver");
  request = createWebDriverClient(`http://127.0.0.1:${port}`);
  const session = await request("POST", "/session", { capabilities: { alwaysMatch: {
    browserName: "MicrosoftEdge", "ms:edgeOptions": { binary: edgeExecutable,
      args: ["--headless=new", "--disable-gpu", "--no-first-run", "--window-size=900,800"] },
  } } });
  sessionId = session.sessionId;
  const execute = (script, args = []) => request("POST", `/session/${sessionId}/execute/sync`, { script, args });
  await request("POST", `/session/${sessionId}/url`, { url: `http://127.0.0.1:${server.address().port}/` });
  const scenarios = [
    { id: "missing-original", state: missing, rows: 1 },
    { id: "processing", state: { kind: "imageProcessingProgress", progress: { kind: "determinate", completed: 0, total: 1, status: "Preparando a Foto…" } }, rows: 0 },
    { id: "problems-after-progress", state: missing, rows: 1 },
    { id: "long-problem-list", state: { ...missing, problems: Array.from({ length: 15 }, (_, index) => ({ ...missing.problems[0], mediaId: `photo-${index}`, fileName: `Foto ${index}.jpg` })) }, rows: 15 },
    { id: "recovered", state: { ...missing, problems: [] }, rows: 0 },
  ];
  for (const [index, scenario] of scenarios.entries()) {
    if (index > 0) await execute(`
      window.fitting.fits=[]; window.fitting.lastFit=Date.now();
      window.fitting.presentation.state=arguments[0];
      document.querySelector('iframe').contentWindow.presentFittingState(window.fitting.presentation);`, [scenario.state]);
    let result;
    const deadline = Date.now() + 4_500;
    do {
      await new Promise(resolve => setTimeout(resolve, 100));
      result = await execute(`
        const frame=document.querySelector('iframe'),doc=frame.contentDocument;
        const scroll=doc.querySelector('.ui-problems-scroll'),row=doc.querySelector('tbody tr');
        const box=scroll?.getBoundingClientRect(),rowBox=row?.getBoundingClientRect();
        const footer=doc.querySelector('.ui-dialog-window__footer')?.getBoundingClientRect();
        return {...window.fitting,height:frame.clientHeight,rows:doc.querySelectorAll('tbody tr').length,
          firstRowVisible:rowBox?Math.max(0,Math.min(rowBox.bottom,box.bottom)-Math.max(rowBox.top,box.top)):0,
          rowHeight:rowBox?.height??0,footerBottom:footer?.bottom??0,
          scrollHeight:scroll?.clientHeight??0,screenLimit:frame.contentWindow.screen.availHeight-64};`);
      if (result.ready && result.fits.length > 0 && Date.now() - result.lastFit > 250) break;
    } while (Date.now() < deadline);
    results.push({ id: scenario.id, ...result });
    writeFileSync(path.join(output, "results.json"), JSON.stringify(results, null, 2));
    writeFileSync(path.join(output, `${scenario.id}.png`), Buffer.from(await request("GET", `/session/${sessionId}/screenshot`), "base64"));
    console.log(JSON.stringify({ id: scenario.id, height: result.height, resizeCount: result.fits.length, firstFits: result.fits.slice(0, 5), lastFits: result.fits.slice(-5), rows: result.rows, firstRowVisible: result.firstRowVisible }));
    assert.deepEqual(result.errors, [], "The real dialog must render without errors");
    assert.equal(result.rows, scenario.rows, "Problem data must reach the real dialog");
    assert.ok(result.fits.length > 0 && result.fits.length <= 4, "Automatic fitting must settle without a shrinking loop");
    assert.ok(result.height <= result.screenLimit, "The dialog must respect the available screen height");
    assert.ok(result.footerBottom <= result.height, "The footer must remain inside the native viewport");
    if (scenario.rows) assert.ok(result.firstRowVisible >= Math.min(result.rowHeight, 50), "The first problem must remain visible after automatic fitting");
  }
} finally {
  if (sessionId) await request("DELETE", `/session/${sessionId}`).catch(() => {});
  child.kill();
  server.close();
}
