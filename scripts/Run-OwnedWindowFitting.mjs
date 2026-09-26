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
const manifest = JSON.parse(readFileSync("src/test/uiAcceptanceScenarios.json", "utf8"));
const openingScenario = manifest.scenarios.find(scenario => scenario.id === "normal-export-whole-jpeg");
const opening = JSON.parse(new URL(openingScenario.implementationPath, "http://localhost").searchParams.get("state"));
const presentation = { sessionId: "owned-window-fitting", state: opening, windowWidth: 800 };

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
      if (command === "fit_owned_window") {
        const size = args;
        if (!size || !Number.isFinite(size.height)) throw new Error("Invalid native size request");
        parent.fitting.fits.push(size.height);
        if (parent.fitting.ready) parent.fitting.visibleFits.push(size.height);
        parent.fitting.lastFit = Date.now();
        parent.document.querySelector("iframe").style.height = `${size.height}px`;
        parent.document.querySelector("iframe").style.width = `${size.width}px`;
        // Native command completion does not wait for the browser's next layout.
        return;
      }
      if (command === "owned_window_content_ready") {
        parent.fitting.ready++;
        parent.fitting.readyHeight = parent.document.querySelector("iframe").clientHeight;
        return;
      }
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
    response.end(`<html><body><script>window.fitting={fits:[],visibleFits:[],errors:[],lastFit:0,ready:0,presentation:${JSON.stringify(presentation)}};</script><iframe style="width:800px;height:478px;border:0" src="/project-dialog.html?presentation=${encodeURIComponent(JSON.stringify(presentation))}&ownedReadyToken=1"></iframe></body></html>`);
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
    { id: "export-opening", state: opening, rows: 0 },
    { id: "export-range-empty", interval: "", stableWindow: true, rows: 0, invalid: false },
    { id: "export-range-invalid", interval: "3-2", stableWindow: true, rows: 0, invalid: true },
    { id: "export-range-cleared", interval: "", stableWindow: true, rows: 0, invalid: false },
    { id: "export-range-valid", interval: "1-2", stableWindow: true, rows: 0, invalid: false },
    { id: "export-checking", state: { ...opening, busy: true }, rows: 0, preserveOpeningSize: true },
    { id: "export-conflicts", state: { kind: "exportConflicts", files: ["Album_001.png"] }, rows: 0, width: 520 },
    { id: "missing-original", state: missing, rows: 1, width: 640 },
    { id: "processing", state: { kind: "imageProcessingProgress", progress: { kind: "determinate", completed: 0, total: 1, status: "Preparando a Foto…" } }, rows: 0, width: 440 },
    { id: "problems-after-progress", state: missing, rows: 1, width: 640 },
    { id: "long-problem-list", state: { ...missing, problems: Array.from({ length: 15 }, (_, index) => ({ ...missing.problems[0], mediaId: `photo-${index}`, fileName: `Foto ${index}.jpg` })) }, rows: 15, width: 640 },
    { id: "export-resumed", state: { kind: "exportProgress", cancelRequested: false, cancellable: false,
      progress: { kind: "indeterminate", status: "Iniciando a Exportação" } }, rows: 0, width: 440 },
    { id: "configuration-returned", state: opening, rows: 0, width: 800 },
  ];
  for (const [index, scenario] of scenarios.entries()) {
    if (scenario.stableWindow) await execute(`
      window.fitting.fits=[]; window.fitting.lastFit=Date.now();
      const frameWindow=document.querySelector('iframe').contentWindow,doc=frameWindow.document;
      doc.querySelector('.ui-export-form__range input[type="radio"]').click();
      const input=doc.querySelector('[aria-label="Lâminas do intervalo"]');
      Object.getOwnPropertyDescriptor(frameWindow.HTMLInputElement.prototype,'value').set.call(input,arguments[0]);
      input.dispatchEvent(new frameWindow.Event('input',{bubbles:true}));`, [scenario.interval]);
    else if (index > 0) await execute(`
      window.fitting.fits=[]; window.fitting.lastFit=Date.now();
      window.fitting.presentation.state=arguments[0];
      window.fitting.presentation.windowWidth=arguments[1];
      document.querySelector('iframe').contentWindow.presentFittingState(window.fitting.presentation);`, [scenario.state, scenario.width ?? 800]);
    let result;
    const deadline = Date.now() + 4_500;
    do {
      await new Promise(resolve => setTimeout(resolve, 100));
      result = await execute(`
        const frame=document.querySelector('iframe'),doc=frame.contentDocument;
        const scroll=doc.querySelector('.ui-problems-scroll'),row=doc.querySelector('.ui-problems-list > li');
        const box=scroll?.getBoundingClientRect(),rowBox=row?.getBoundingClientRect();
        const footer=doc.querySelector('.ui-dialog-window__footer')?.getBoundingClientRect();
        const tooltip=doc.querySelector('[role="tooltip"]')?.getBoundingClientRect();
        const body=doc.querySelector('.ui-dialog-window__body')?.getBoundingClientRect();
        return {...window.fitting,width:frame.clientWidth,height:frame.clientHeight,rows:doc.querySelectorAll('.ui-problems-list > li').length,
          intervalInvalid:doc.querySelector('[aria-label="Lâminas do intervalo"]')?.getAttribute('aria-invalid')==='true',
          tooltipVisible:Boolean(tooltip),tooltipContained:!tooltip||(tooltip.top>=body.top&&tooltip.bottom<=body.bottom&&tooltip.left>=0&&tooltip.right<=frame.clientWidth),
          firstRowVisible:rowBox?Math.max(0,Math.min(rowBox.bottom,box.bottom)-Math.max(rowBox.top,box.top)):0,
          rowHeight:rowBox?.height??0,footerBottom:footer?.bottom??0,
          scrollHeight:scroll?.clientHeight??0,screenLimit:frame.contentWindow.screen.availHeight-64};`);
      if (result.ready && (result.fits.length > 0 || scenario.stableWindow) && Date.now() - result.lastFit > 250) break;
    } while (Date.now() < deadline);
    results.push({ id: scenario.id, ...result });
    writeFileSync(path.join(output, "results.json"), JSON.stringify(results, null, 2));
    writeFileSync(path.join(output, `${scenario.id}.png`), Buffer.from(await request("GET", `/session/${sessionId}/screenshot`), "base64"));
    console.log(JSON.stringify({ id: scenario.id, height: result.height, resizeCount: result.fits.length, firstFits: result.fits.slice(0, 5), lastFits: result.fits.slice(-5), rows: result.rows, firstRowVisible: result.firstRowVisible }));
    assert.deepEqual(result.errors, [], "The real dialog must render without errors");
    if (index === 0) {
      assert.equal(result.ready, 1, "The export window must become ready exactly once");
      assert.deepEqual(result.visibleFits, [], "The export window must not resize after becoming visible");
      assert.equal(result.readyHeight, result.height, "The first visible height must match the settled height");
    }
    assert.equal(result.rows, scenario.rows, "Problem data must reach the real dialog");
    assert.equal(result.width, scenario.width ?? 800, "The window must use the width delivered with the current dialog");
    if (scenario.preserveOpeningSize) {
      assert.equal(result.height, results[0].height, "Checking export must not enlarge the configuration window before showing the next dialog");
      assert.deepEqual(result.fits, [], "Checking export must preserve the existing window size");
    }
    if (scenario.stableWindow) {
      assert.equal(result.fits.length, 0, "Range editing and tooltips must not resize the window");
      assert.equal(result.height, results[0].height, "Range editing must preserve the opening height");
      assert.equal(result.intervalInvalid, scenario.invalid, "An empty interval must remain neutral");
      assert.equal(result.tooltipVisible, scenario.invalid, "Only a filled invalid interval needs an error tooltip");
      assert.ok(result.tooltipContained, "The tooltip must remain fully visible inside the dialog");
    } else if (!scenario.preserveOpeningSize) assert.ok(result.fits.length > 0 && result.fits.length <= 4, "Automatic fitting must settle without a shrinking loop");
    assert.ok(result.height <= result.screenLimit, "The dialog must respect the available screen height");
    assert.ok(result.footerBottom <= result.height, "The footer must remain inside the native viewport");
    if (scenario.rows) assert.ok(result.firstRowVisible >= Math.min(result.rowHeight, 50), "The first problem must remain visible after automatic fitting");
  }
} finally {
  if (sessionId) await request("DELETE", `/session/${sessionId}`).catch(() => {});
  child.kill();
  server.close();
}
