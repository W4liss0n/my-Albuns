// Turns the raw evidence of a run into "where the program spends": CPU,
// memory and disk per process role, JavaScript self time per source function
// (minified frames mapped back through the hidden source maps of the same
// build), long-animation-frame scripts, and the timed operations the Rust
// host logs.
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { SourceMap } from "node:module";
import { basename, join } from "node:path";

const SPECIAL = new Set(["(root)", "(program)", "(idle)", "(garbage collector)"]);

// Source maps live in <lab>/sourcemaps/<build>/assets/<bundle>.js.map. Vite
// names bundles by content hash, so a map with the bundle's name belongs to
// exactly that bundle.
export function createSourceResolver(sourceMapRoot) {
  const bundles = new Map();
  if (sourceMapRoot && existsSync(sourceMapRoot)) {
    for (const build of readdirSync(sourceMapRoot)) {
      const assets = join(sourceMapRoot, build, "assets");
      if (!existsSync(assets) || !statSync(assets).isDirectory()) continue;
      for (const name of readdirSync(assets)) {
        if (name.endsWith(".js") && existsSync(join(assets, `${name}.map`))) {
          bundles.set(name, join(assets, name));
        }
      }
    }
  }
  const loaded = new Map();
  const load = (bundle) => {
    if (!loaded.has(bundle)) {
      const path = bundles.get(bundle);
      if (!path) {
        loaded.set(bundle, null);
      } else {
        const source = readFileSync(path, "utf8");
        const lineStarts = [0];
        for (let index = 0; index < source.length; index += 1) {
          if (source.charCodeAt(index) === 10) lineStarts.push(index + 1);
        }
        loaded.set(bundle, {
          map: new SourceMap(JSON.parse(readFileSync(`${path}.map`, "utf8"))),
          lineStarts,
        });
      }
    }
    return loaded.get(bundle);
  };
  const tidy = (source) =>
    source
      .replace(/^(\.\.\/)+/, "")
      .replace(/^.*node_modules\//, "node_modules/")
      .replace(/\?.*$/, "");

  return {
    available: bundles.size > 0,
    // lineNumber and columnNumber are zero-based, as in CPU profiles.
    resolve(url, lineNumber, columnNumber, fallbackName) {
      const bundle = basename((url ?? "").replace(/[?#].*$/, ""));
      const entry = bundle && load(bundle)?.map.findEntry(lineNumber, columnNumber);
      if (!entry || !entry.originalSource) {
        return { name: fallbackName || "(anônima)", where: bundle ? `${bundle}:${lineNumber + 1}` : "(nativo)" };
      }
      const source = tidy(entry.originalSource);
      return {
        // The map names the identifier at the function start; the profile's
        // own name is minified.
        name: entry.name || fallbackName || "(anônima)",
        source,
        where: `${source}:${entry.originalLine + 1}`,
      };
    },
    resolveCharacter(url, position, fallbackName) {
      const bundle = basename((url ?? "").replace(/[?#].*$/, ""));
      const data = bundle && load(bundle);
      if (!data || position == null || position < 0) return this.resolve(url, -1, -1, fallbackName);
      let line = 0;
      let high = data.lineStarts.length - 1;
      while (line < high) {
        const middle = Math.ceil((line + high) / 2);
        if (data.lineStarts[middle] <= position) line = middle;
        else high = middle - 1;
      }
      return this.resolve(url, line, position - data.lineStarts[line], fallbackName);
    },
  };
}

// Self time per CPU-profile node, from the exact sample deltas.
export function analyzeCpuProfile(profile, resolver) {
  const nodes = new Map(profile.nodes.map((node) => [node.id, node]));
  const self = new Map();
  profile.samples.forEach((id, index) => {
    self.set(id, (self.get(id) ?? 0) + (profile.timeDeltas[index] ?? 0) / 1000);
  });
  const totals = { sampledMs: 0, idleMs: 0, programMs: 0, gcMs: 0 };
  const functions = new Map();
  const files = new Map();
  for (const [id, ms] of self) {
    const frame = nodes.get(id)?.callFrame;
    if (!frame) continue;
    totals.sampledMs += ms;
    if (frame.functionName === "(idle)") totals.idleMs += ms;
    else if (frame.functionName === "(program)") totals.programMs += ms;
    else if (frame.functionName === "(garbage collector)") totals.gcMs += ms;
    if (SPECIAL.has(frame.functionName)) continue;
    const resolved = resolver.resolve(frame.url, frame.lineNumber, frame.columnNumber, frame.functionName);
    const key = `${resolved.name} @ ${resolved.where}`;
    functions.set(key, (functions.get(key) ?? 0) + ms);
    const file = resolved.source ?? (frame.url ? basename(frame.url) : "(nativo do navegador)");
    files.set(file, (files.get(file) ?? 0) + ms);
  }
  const top = (map, count) =>
    [...map.entries()].sort((left, right) => right[1] - left[1]).slice(0, count);
  return { totals, functions: top(functions, 12), files: top(files, 8) };
}

// Long animation frames list the scripts that ran in them with their time.
export function analyzeAnimationFrames(animationFrames, resolver) {
  const scripts = new Map();
  for (const frame of animationFrames ?? []) {
    for (const [url, functionName, invoker, duration, position] of frame[3] ?? []) {
      const resolved = resolver.resolveCharacter(url, position, functionName);
      const key = `${invoker || "?"} → ${resolved.name} @ ${resolved.where}`;
      const entry = scripts.get(key) ?? { count: 0, ms: 0 };
      entry.count += 1;
      entry.ms += duration ?? 0;
      scripts.set(key, entry);
    }
  }
  return [...scripts.entries()].sort((left, right) => right[1].ms - left[1].ms).slice(0, 8);
}

export function readRoles(runDirectory) {
  const path = join(runDirectory, "roles.csv");
  if (!existsSync(path)) return [];
  const [header, ...lines] = readFileSync(path, "utf8").trim().split(/\r?\n/);
  const columns = header.split(",");
  return lines.map((line) => {
    const values = line.split(",");
    return Object.fromEntries(
      columns.map((column, index) => [
        column,
        index === 0 ? Date.parse(values[0]) : index === 1 ? values[1] : Number(values[index]),
      ]),
    );
  });
}

// CPU seconds, peak memory and disk traffic per role inside a time window.
export function summarizeRoles(rows, logicalProcessors) {
  const byRole = new Map();
  const ordered = [...rows].sort((left, right) => left.timestamp - right.timestamp);
  let previous = new Map();
  for (const row of ordered) {
    const last = previous.get(row.role);
    const seconds = last ? Math.min(5, (row.timestamp - last) / 1000) : 1;
    previous.set(row.role, row.timestamp);
    const entry = byRole.get(row.role) ?? { cpuSeconds: 0, peakPrivateMB: 0, readMB: 0, writeMB: 0, peakProcesses: 0 };
    entry.cpuSeconds += (row.cpuPercent / 100) * logicalProcessors * seconds;
    entry.peakPrivateMB = Math.max(entry.peakPrivateMB, row.privateMB);
    entry.readMB += row.readMB;
    entry.writeMB += row.writeMB;
    entry.peakProcesses = Math.max(entry.peakProcesses, row.processes);
    byRole.set(row.role, entry);
  }
  return [...byRole.entries()].sort((left, right) => right[1].cpuSeconds - left[1].cpuSeconds);
}

// Timed operations the Rust host writes to its log.
export function summarizeTimedEvents(events) {
  const groups = new Map();
  for (const event of events) {
    const ms = event.elapsed_ms ?? event.waited_ms;
    if (typeof ms !== "number") continue;
    const key = `${event.event} (${event.elapsed_ms != null ? "duração" : "espera"})`;
    const list = groups.get(key) ?? [];
    list.push(ms);
    groups.set(key, list);
  }
  return [...groups.entries()]
    .map(([key, values]) => {
      const sorted = [...values].sort((left, right) => left - right);
      return {
        key,
        count: values.length,
        totalMs: values.reduce((sum, value) => sum + value, 0),
        p95: sorted[Math.min(sorted.length - 1, Math.floor(0.95 * sorted.length))],
        max: sorted.at(-1),
      };
    })
    .sort((left, right) => right.totalMs - left.totalMs);
}
