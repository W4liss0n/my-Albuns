// Runs inside the lab VM with a copied node.exe. It finds the Project
// WebView through its DevTools port, waits for the canvas, then drives wheel
// scrolling and Ctrl+wheel zoom while recording animation frames, long tasks
// and long animation frames. The result is always written, also on failure,
// so the host still collects logs and counters.
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";

// PowerShell writes the config with a byte-order mark.
const config = JSON.parse(readFileSync(process.argv[2], "utf8").replace(/^﻿/, ""));
const result = {
  launchedAt: config.launchedAt,
  startedAt: Date.now(),
  steps: [],
  phases: [],
  profiles: [],
  errors: [],
};

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const sinceLaunch = () => Date.now() - config.launchedAt;

class Cdp {
  static async connect(url) {
    const socket = new WebSocket(url);
    await new Promise((resolve, reject) => {
      socket.onopen = resolve;
      socket.onerror = () => reject(new Error(`could not connect to ${url}`));
    });
    return new Cdp(socket);
  }

  constructor(socket) {
    this.socket = socket;
    this.nextId = 1;
    this.pending = new Map();
    this.closed = false;
    socket.onmessage = (message) => {
      const payload = JSON.parse(message.data);
      const pending = payload.id && this.pending.get(payload.id);
      if (!pending) return;
      this.pending.delete(payload.id);
      clearTimeout(pending.timer);
      if (payload.error) pending.reject(new Error(payload.error.message));
      else pending.resolve(payload.result);
    };
    socket.onclose = () => {
      this.closed = true;
      for (const pending of this.pending.values()) {
        clearTimeout(pending.timer);
        pending.reject(new Error("the DevTools connection closed"));
      }
      this.pending.clear();
    };
  }

  send(method, params = {}, timeoutMs = 30_000) {
    if (this.closed) return Promise.reject(new Error("the DevTools connection closed"));
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`${method} did not answer within ${timeoutMs} ms`));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  async evaluate(expression, timeoutMs = 30_000) {
    const response = await this.send(
      "Runtime.evaluate",
      { expression, returnByValue: true, awaitPromise: true },
      timeoutMs,
    );
    if (response.exceptionDetails) {
      throw new Error(
        response.exceptionDetails.exception?.description ?? response.exceptionDetails.text,
      );
    }
    return response.result.value;
  }

  close() {
    this.socket.close();
  }
}

// The Project window stays hidden until its images are prepared, but its
// page answers DevTools before that and reports itself visible. The Hosts'
// log says when each window was shown; returns those times in order.
function projectWindowPresentations() {
  const root = config.logDirectory;
  const times = [];
  if (!root || !existsSync(root)) return times;
  for (const name of readdirSync(root)) {
    if (!/^myalbuns-desktop\..*\.jsonl$/.test(name)) continue;
    let text;
    try {
      text = readFileSync(join(root, name), "utf8");
    } catch {
      continue;
    }
    for (const line of text.split(/\r?\n/)) {
      if (!line.includes('"project_window_presented"')) continue;
      try {
        const at = Date.parse(JSON.parse(line).timestamp);
        if (at >= config.launchedAt) times.push(at);
      } catch {
        // A line still being written.
      }
    }
  }
  return times.sort((left, right) => left - right);
}

const projectWindowPresentedAt = () => projectWindowPresentations()[0] ?? null;

function activePortFiles() {
  const root = config.webviewStateRoot;
  if (!existsSync(root)) return [];
  return readdirSync(root)
    .map((name) => join(root, name, "EBWebView", "DevToolsActivePort"))
    .filter((path) => existsSync(path));
}

function listeningWebviewPorts() {
  try {
    const tasks = execFileSync(
      "tasklist",
      ["/FI", "IMAGENAME eq msedgewebview2.exe", "/FO", "CSV", "/NH"],
      { encoding: "utf8" },
    );
    const pids = new Set(
      tasks
        .split(/\r?\n/)
        .map((line) => line.split('","')[1])
        .filter(Boolean),
    );
    const netstat = execFileSync("netstat", ["-ano", "-p", "TCP"], { encoding: "utf8" });
    return netstat
      .split(/\r?\n/)
      .map((line) => line.trim().split(/\s+/))
      .filter((parts) => parts.length >= 5 && parts[1].startsWith("127.0.0.1:") && pids.has(parts[4]))
      .map((parts) => Number(parts[1].split(":")[1]));
  } catch {
    return [];
  }
}

function candidatePorts() {
  const ports = new Set();
  for (const path of activePortFiles()) {
    // WebView2 holds the file while it writes it; the next poll reads it.
    let text;
    try {
      text = readFileSync(path, "utf8");
    } catch {
      continue;
    }
    const port = Number(text.split(/\r?\n/)[0]);
    if (port > 0) ports.add(port);
  }
  for (const port of listeningWebviewPorts()) ports.add(port);
  return [...ports];
}

// The Project page is served at the app root; the opening progress is a
// dialog page of its own.
const isProjectPage = (target) =>
  target.type === "page" && /^https?:\/\/tauri\.localhost(?::\d+)?\/(?:index\.html)?(?:[#?].*)?$/.test(target.url);
const isOpeningDialog = (target) => target.type === "page" && target.url.includes("kind=opening-project");

async function listPages() {
  const pages = [];
  for (const port of candidatePorts()) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`, {
        signal: AbortSignal.timeout(3_000),
      });
      for (const target of await response.json()) pages.push({ port, target });
    } catch {
      // A WebView that is starting or closing; try the next port.
    }
  }
  return pages;
}

async function evaluateOnce(target, expression) {
  const cdp = await Cdp.connect(target.webSocketDebuggerUrl);
  try {
    return await cdp.evaluate(expression, 10_000);
  } finally {
    cdp.close();
  }
}

const READY_PROBE = `(() => {
  const host = document.querySelector(".canvas-host");
  const alert = document.querySelector('[role="alert"]');
  return {
    ready: Boolean(host?.querySelector("canvas")) && !document.querySelector(".canvas-loading"),
    visibility: document.visibilityState,
    focused: document.hasFocus(),
    alert: alert ? alert.textContent.trim().slice(0, 300) : null,
  };
})()`;

const INSTRUMENT = `(() => {
  if (window.__hardwareLab) return true;
  const lab = { frames: [], longTasks: [], animationFrames: [] };
  window.__hardwareLab = lab;
  const tick = (timestamp) => {
    lab.frames.push(timestamp);
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  try {
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) lab.longTasks.push([entry.startTime, entry.duration]);
    }).observe({ type: "longtask", buffered: true });
  } catch {}
  try {
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        lab.animationFrames.push([
          entry.startTime,
          entry.duration,
          entry.blockingDuration ?? 0,
          (entry.scripts ?? []).map((script) => [
            script.sourceURL,
            script.sourceFunctionName,
            script.invoker,
            Math.round(script.duration),
            script.sourceCharPosition,
          ]),
        ]);
      }
    }).observe({ type: "long-animation-frame", buffered: true });
  } catch {}
  return true;
})()`;

// CPU profiles of the Project page, saved as .cpuprofile files that DevTools
// opens and the summary attributes to source files through source maps.
async function startProfile(cdp) {
  await cdp.send("Profiler.enable");
  await cdp.send("Profiler.setSamplingInterval", { interval: 1000 });
  await cdp.send("Profiler.start");
}

async function saveProfile(cdp, name) {
  try {
    const { profile } = await cdp.send("Profiler.stop", {}, 60_000);
    const path = join(config.profileDirectory, `profile-${config.iteration}-${name}.cpuprofile`);
    writeFileSync(path, JSON.stringify(profile));
    result.profiles.push({ name, file: basename(path) });
  } catch (error) {
    result.errors.push(`profile ${name}: ${error.message}`);
  }
}

const OPENING_PROBE = `({ visibility: document.visibilityState, text: document.body.innerText.trim().replace(/\\s+/g, " ") })`;

// Ready means the opening dialog is gone (or hidden) and the Project canvas is
// drawn. While the dialog shows, its text ("Preparando imagens 34% 101 de
// 300") is sampled so the summary can draw the preparation curve.
async function waitForCanvas(deadline) {
  result.openingProgress = [];
  let cdp = null;
  let lastProgressAt = 0;
  let last = null;
  let segment = 1;
  const step = (name, extra = {}) => {
    if (!result.steps.some((existing) => existing.name === name)) {
      result.steps.push({ name, sinceLaunchMs: sinceLaunch(), ...extra });
    }
  };
  while (Date.now() < deadline) {
    const pages = await listPages();
    if (pages.length) step("devtools_connected");
    const opening = pages.find((page) => isOpeningDialog(page.target));
    let openingVisible = false;
    if (opening) {
      try {
        const state = await evaluateOnce(opening.target, OPENING_PROBE);
        openingVisible = state.visibility === "visible";
        if (openingVisible) {
          step("opening_dialog_seen");
          if (Date.now() - lastProgressAt >= config.progressIntervalMs) {
            result.openingProgress.push({ sinceLaunchMs: sinceLaunch(), text: state.text.slice(0, 200) });
            lastProgressAt = Date.now();
          }
        }
      } catch {
        // The dialog is closing.
      }
    }
    if (!openingVisible && result.steps.some((existing) => existing.name === "opening_dialog_seen")) {
      step("opening_dialog_closed");
    }
    const presentedAt = config.logDirectory ? projectWindowPresentedAt() : Date.now();
    const projects = pages.filter((page) => isProjectPage(page.target));
    let project = projects[0];
    // Of several Projects, the first editor shown takes the focus.
    if (projects.length > 1 && presentedAt !== null) {
      for (const candidate of projects) {
        try {
          if (await evaluateOnce(candidate.target, "document.hasFocus()")) {
            project = candidate;
            break;
          }
        } catch {
          // A page that is closing.
        }
      }
    }
    if (project) {
      try {
        // The Project page loads behind the opening dialog; profile it from
        // the first connection. A replaced WebView starts a new segment.
        if (!cdp || cdp.closed || cdp.targetId !== project.target.id) {
          if (cdp && !cdp.closed) await saveProfile(cdp, `abertura-trecho-${segment}`);
          cdp?.close();
          cdp = await Cdp.connect(project.target.webSocketDebuggerUrl);
          cdp.targetId = project.target.id;
          await cdp.send("Performance.enable");
          await startProfile(cdp);
          segment += 1;
        }
        if (presentedAt !== null) {
          if (!result.steps.some((existing) => existing.name === "project_window_presented")) {
            result.steps.push({ name: "project_window_presented", sinceLaunchMs: presentedAt - config.launchedAt });
          }
        }
        if (!openingVisible && presentedAt !== null) {
          last = await cdp.evaluate(READY_PROBE, 10_000);
          if (last.alert) result.lastAlert = last.alert;
          if (last.ready) {
            step("canvas_ready", last);
            await saveProfile(cdp, "abertura");
            return cdp;
          }
        }
      } catch (error) {
        // The Host replaces the Project WebView during opening; reconnect.
        if (!cdp?.closed) result.errors.push(`ready probe: ${error.message}`);
      }
    }
    await sleep(opening ? 2_000 : 500);
  }
  result.lastProbe = last;
  throw new Error(
    result.steps.length
      ? "the Project did not finish opening before the deadline"
      : "no app WebView exposed a DevTools port; WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS may be ignored",
  );
}

const pageNow = (cdp) => cdp.evaluate("performance.now()");

async function canvasCenter(cdp) {
  return cdp.evaluate(`(() => {
    const rect = document.querySelector(".canvas-host").getBoundingClientRect();
    return { x: Math.round(rect.left + rect.width / 2), y: Math.round(rect.top + rect.height / 2) };
  })()`);
}

async function wheel(cdp, point, deltaY, modifiers, count, spacingMs, latencies) {
  for (let index = 0; index < count; index += 1) {
    const started = performance.now();
    await cdp.send("Input.dispatchMouseEvent", {
      type: "mouseWheel",
      x: point.x,
      y: point.y,
      deltaX: 0,
      deltaY,
      modifiers,
    });
    latencies.push(performance.now() - started);
    await sleep(spacingMs);
  }
}

// A phase records its own CPU profile and page-clock window. An action that
// fails is recorded in the phase details so the next phases still run.
async function phase(cdp, name, action) {
  await startProfile(cdp);
  const start = await pageNow(cdp);
  const latencies = [];
  const details = {};
  try {
    await action(latencies, details);
  } catch (error) {
    details.error = error.message;
  }
  const end = await pageNow(cdp);
  await saveProfile(cdp, name);
  const metrics = await cdp.send("Performance.getMetrics");
  result.phases.push({
    name,
    start,
    end,
    inputLatencies: latencies,
    details,
    metrics: Object.fromEntries(metrics.metrics.map((metric) => [metric.name, metric.value])),
  });
}

// Scrolls an element into the middle of the page and returns its centre, or
// null when the selector matches nothing visible.
async function centerOf(cdp, selector) {
  return cdp.evaluate(`(() => {
    const element = document.querySelector(${JSON.stringify(selector)});
    if (!element) return null;
    element.scrollIntoView({ block: "center", inline: "center" });
    const rect = element.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    return { x: Math.round(rect.left + rect.width / 2), y: Math.round(rect.top + rect.height / 2) };
  })()`);
}

async function click(cdp, point, clickCount = 1) {
  await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: point.x, y: point.y });
  for (let count = 1; count <= clickCount; count += 1) {
    await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x: point.x, y: point.y, button: "left", clickCount: count });
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: point.x, y: point.y, button: "left", clickCount: count });
  }
}

async function press(cdp, key, code, keyCode, modifiers = 0) {
  const base = { key, code, windowsVirtualKeyCode: keyCode, nativeVirtualKeyCode: keyCode, modifiers };
  await cdp.send("Input.dispatchKeyEvent", { type: "rawKeyDown", ...base });
  await cdp.send("Input.dispatchKeyEvent", { type: "keyUp", ...base });
}

// Milliseconds until the page expression is truthy, or null on timeout.
async function waitFor(cdp, expression, timeoutMs) {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    if (await cdp.evaluate(`Boolean(${expression})`)) return Date.now() - started;
    await sleep(100);
  }
  return null;
}

async function run() {
  const cdp = await waitForCanvas(Date.now() + config.readyTimeoutMs);
  await cdp.evaluate(INSTRUMENT);
  const point = await canvasCenter(cdp);
  const ctrl = 2;
  await phase(cdp, "ocioso_apos_abrir", () => sleep(config.idleMs));
  await phase(cdp, "rolagem", async (latencies) => {
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: point.x, y: point.y });
    await wheel(cdp, point, 120, 0, 30, 60, latencies);
    await wheel(cdp, point, -120, 0, 30, 60, latencies);
  });
  await phase(cdp, "zoom", async (latencies) => {
    await wheel(cdp, point, -120, ctrl, 10, 80, latencies);
    await wheel(cdp, point, 120, ctrl, 10, 80, latencies);
  });

  // Double-clicking a Sheet Grid tile centres that Sheet on the canvas.
  await phase(cdp, "navegar_laminas", async (latencies, details) => {
    const ids = await cdp.evaluate(
      `[...document.querySelectorAll('.sheet-grid [data-sheet-id]')].map((element) => element.dataset.sheetId)`,
    );
    if (!ids.length) throw new Error("a grade de lâminas não está na tela");
    details.navigations = [];
    for (const index of [8, 20, 35, 0].filter((position) => position < ids.length)) {
      const tile = await centerOf(cdp, `.sheet-grid [data-sheet-id="${ids[index]}"] > button`);
      if (!tile) continue;
      const started = performance.now();
      await click(cdp, tile, 2);
      latencies.push(performance.now() - started);
      const centredMs = await waitFor(
        cdp,
        `document.querySelector(".canvas-host")?.dataset.centeredSheetId === ${JSON.stringify(ids[index])}`,
        10_000,
      );
      details.navigations.push({ sheet: index + 1, centredMs });
      await sleep(1_500);
    }
  });

  await phase(cdp, "grade_rolagem", async (latencies, details) => {
    const grid = await centerOf(cdp, ".sheet-grid");
    if (!grid) throw new Error("a grade de lâminas não está na tela");
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: grid.x, y: grid.y });
    await wheel(cdp, grid, 120, 0, 15, 80, latencies);
    await wheel(cdp, grid, -120, 0, 15, 80, latencies);
    details.wheelEvents = latencies.length;
  });

  // Opening the panel generates layout suggestions for the Sheet; applying one
  // rebuilds its Frames. Both are undone before saving.
  await phase(cdp, "painel_de_layouts", async (latencies, details) => {
    const toggle = `button[aria-label^="Layouts da lâmina"]:not([disabled])`;
    const button = await centerOf(cdp, toggle);
    if (!button) throw new Error("nenhum botão de layouts habilitado");
    details.sheet = await cdp.evaluate(`document.querySelector(${JSON.stringify(toggle)}).getAttribute("aria-label")`);
    await click(cdp, button);
    details.suggestionsMs = await waitFor(cdp, `document.querySelector("#layout-panel .layout-panel__preview:not([disabled])")`, 30_000);
    details.suggestions = await cdp.evaluate(`document.querySelectorAll("#layout-panel .layout-panel__preview").length`);
    const suggestion = await centerOf(cdp, `#layout-panel button[aria-label^="Aplicar layout 2"]`);
    if (suggestion) {
      await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: suggestion.x, y: suggestion.y });
      await sleep(600);
      const started = performance.now();
      await click(cdp, suggestion);
      latencies.push(performance.now() - started);
      details.applied = true;
      await sleep(1_500);
    }
    const open = await centerOf(cdp, `button[aria-label^="Layouts da lâmina"][aria-expanded="true"]`);
    if (open) await click(cdp, open);
    details.closedMs = await waitFor(cdp, `!document.querySelector("#layout-panel")`, 5_000);
  });

  // The media panel starts a pointer drag; dropping on the canvas centre
  // swaps or places the Photo.
  await phase(cdp, "arrastar_foto", async (latencies, details) => {
    const item = await centerOf(cdp, "[data-media-id]");
    if (!item) throw new Error("nenhuma foto no painel de imagens");
    const target = await canvasCenter(cdp);
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x: item.x, y: item.y });
    await cdp.send("Input.dispatchMouseEvent", { type: "mousePressed", x: item.x, y: item.y, button: "left", clickCount: 1 });
    details.dragSeen = false;
    for (let step = 1; step <= 20; step += 1) {
      const x = Math.round(item.x + ((target.x - item.x) * step) / 20);
      const y = Math.round(item.y + ((target.y - item.y) * step) / 20);
      const started = performance.now();
      await cdp.send("Input.dispatchMouseEvent", { type: "mouseMoved", x, y, button: "left", buttons: 1 });
      latencies.push(performance.now() - started);
      if (!details.dragSeen) details.dragSeen = await cdp.evaluate(`document.querySelector(".canvas-host")?.hasAttribute("data-media-drag") ?? false`);
      await sleep(30);
    }
    await cdp.send("Input.dispatchMouseEvent", { type: "mouseReleased", x: target.x, y: target.y, button: "left", clickCount: 1 });
    await sleep(1_500);
  });

  // Undo whatever the scenario changed and save, so the next opening of the
  // same project starts clean instead of offering recovery.
  await phase(cdp, "desfazer_e_salvar", async (latencies, details) => {
    await press(cdp, "Escape", "Escape", 27);
    for (let count = 0; count < 3; count += 1) {
      const started = performance.now();
      await press(cdp, "z", "KeyZ", 90, ctrl);
      latencies.push(performance.now() - started);
      await sleep(400);
    }
    await press(cdp, "s", "KeyS", 83, ctrl);
    await sleep(3_000);
    details.saved = await cdp.evaluate(`document.body.innerText.includes("Salvo")`);
  });

  await phase(cdp, "assentamento", () => sleep(config.settleMs));

  const recorded = await cdp.evaluate(
    "({ frames: __hardwareLab.frames, longTasks: __hardwareLab.longTasks, animationFrames: __hardwareLab.animationFrames, timeOrigin: performance.timeOrigin })",
  );
  Object.assign(result, recorded);
  result.gpuRenderer = await cdp.evaluate(`(() => {
    const gl = document.createElement("canvas").getContext("webgl2");
    if (!gl) return null;
    const info = gl.getExtension("WEBGL_debug_renderer_info");
    const renderer = gl.getParameter(info ? info.UNMASKED_RENDERER_WEBGL : gl.RENDERER);
    gl.getExtension("WEBGL_lose_context")?.loseContext();
    return renderer;
  })()`);
  try {
    const shot = await cdp.send("Page.captureScreenshot", { format: "png" }, 15_000);
    writeFileSync(config.screenshotPath, Buffer.from(shot.data, "base64"));
  } catch (error) {
    result.errors.push(`screenshot: ${error.message}`);
  }
  cdp.close();

  // With several Projects, the others are still being prepared: wait for
  // their editors too, so the run says when each one appeared.
  const expected = config.projectCount ?? 1;
  const deadline = Date.now() + config.readyTimeoutMs;
  let presented = projectWindowPresentations();
  while (presented.length < expected && Date.now() < deadline) {
    await sleep(1_000);
    presented = projectWindowPresentations();
  }
  result.presentations = presented.map((at) => at - config.launchedAt);
}

try {
  await run();
} catch (error) {
  result.errors.push(error.message);
} finally {
  result.finishedAt = Date.now();
  writeFileSync(config.outputPath, JSON.stringify(result));
}
