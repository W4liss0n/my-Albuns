// Summarizes one hardware-lab run folder into summary.md: opening times from
// the app log, frame and long-task statistics from the DevTools driver, and
// peaks from the guest counters.
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import {
  analyzeAnimationFrames,
  analyzeCpuProfile,
  createSourceResolver,
  readRoles,
  summarizeRoles,
  summarizeTimedEvents,
} from "./profile-analysis.mjs";

const runDirectory = process.argv[2];
// Hidden source maps of the shipped build: <lab>/sourcemaps/<build>/assets.
const resolver = createSourceResolver(process.argv[3] ?? resolve(runDirectory, "..", "..", "sourcemaps"));
const roles = readRoles(runDirectory);
const readJson = (path) => JSON.parse(readFileSync(path, "utf8").replace(/^﻿/, ""));
const run = readJson(join(runDirectory, "run.json"));

function percentile(values, fraction) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.min(sorted.length - 1, Math.floor(fraction * sorted.length))];
}

const ms = (value) => (value == null ? "—" : `${Math.round(value).toLocaleString("pt-BR")} ms`);
const seconds = (value) => (value == null ? "—" : `${(value / 1000).toFixed(1).replace(".", ",")} s`);
const mb = (value) => (value == null ? "—" : `${Math.round(value).toLocaleString("pt-BR")} MB`);
const pct = (value) => (value == null ? "—" : `${Math.round(value)}%`);

function readLog() {
  const directory = join(runDirectory, "logs");
  if (!existsSync(directory)) return [];
  return readdirSync(directory)
    .filter((name) => name.endsWith(".jsonl"))
    .flatMap((name) => readFileSync(join(directory, name), "utf8").split(/\r?\n/))
    .filter((line) => line.startsWith("{"))
    .map((line) => JSON.parse(line))
    .map((entry) => ({ ...entry, at: Date.parse(entry.timestamp) }));
}

function readCounters() {
  const path = join(runDirectory, "counters.csv");
  if (!existsSync(path)) return [];
  const [header, ...lines] = readFileSync(path, "utf8").trim().split(/\r?\n/);
  const columns = header.split(",");
  return lines.map((line) => {
    const values = line.split(",");
    return Object.fromEntries(
      columns.map((column, index) => [column, index === 0 ? Date.parse(values[0]) : Number(values[index])]),
    );
  });
}

function frameStats(frames, start, end) {
  const inside = frames.filter((timestamp) => timestamp >= start && timestamp <= end);
  const deltas = inside.slice(1).map((timestamp, index) => timestamp - inside[index]);
  return {
    p50: percentile(deltas, 0.5),
    p95: percentile(deltas, 0.95),
    max: deltas.length ? Math.max(...deltas) : null,
    over50: deltas.filter((delta) => delta > 50).length,
    over100: deltas.filter((delta) => delta > 100).length,
  };
}

function taskStats(entries, start, end) {
  const inside = entries.filter(([startTime]) => startTime >= start && startTime <= end);
  return {
    count: inside.length,
    total: inside.reduce((sum, [, duration]) => sum + duration, 0),
    max: inside.length ? Math.max(...inside.map(([, duration]) => duration)) : null,
  };
}

const log = readLog();
const counters = readCounters();
const lines = [];
const profile = run.profile;
lines.push(`# Laboratório de hardware — perfil \`${profile.name}\``);
lines.push("");
lines.push(
  `${profile.description}: ${profile.processorCount} vCPU limitadas a ${profile.processorLimitPercent}%, ` +
    `${mb(profile.memoryMB)}, disco do sistema ${profile.systemDiskMaximumIops || "sem limite"} IOPS, ` +
    `disco de dados ${profile.dataDiskMaximumIops || "sem limite"} IOPS.`,
);
lines.push("");
lines.push(`- Instalador: \`${run.installer}\` (versão ${run.install?.version ?? "—"})`);
lines.push(`- Projeto: \`${run.project}\``);
lines.push(`- Convidado: ${run.guest?.os ?? "—"} build ${run.guest?.build ?? "—"}, WebView2 ${run.guest?.webview2 ?? "—"}`);
lines.push(
  `- Runtime do Visual C++ no convidado: ${run.vcRuntime ?? "ausente (Windows limpo; o Processador de Imagens não inicia)"}`,
);
lines.push(`- Vídeo no convidado:${(run.guest?.videoControllers ?? []).join("; ") || "—"}`);
lines.push("");

const iterations = run.iterations ?? [];
iterations.forEach((iteration, position) => {
  const scenarioPath = join(runDirectory, `scenario-${iteration.index}.json`);
  const scenario = existsSync(scenarioPath) ? readJson(scenarioPath) : null;
  const launchedAt = scenario?.launchedAt ?? Date.parse(iteration.launchedAt);
  const next = iterations[position + 1];
  const windowEnd = next ? Date.parse(next.launchedAt) : Number.POSITIVE_INFINITY;
  const events = log.filter((entry) => entry.at >= launchedAt && entry.at < windowEnd);
  const first = (name) => {
    const event = events.find((entry) => entry.event === name);
    return event ? event.at - launchedAt : null;
  };
  const count = (name) => events.filter((entry) => entry.event === name).length;
  const previews = events.filter((entry) => entry.event === "media_preview_completed");

  lines.push(`## Abertura ${iteration.index} (${iteration.kind})`);
  lines.push("");
  if (scenario?.gpuRenderer) lines.push(`Renderizador WebGL2: ${scenario.gpuRenderer}`);
  if (scenario?.errors?.length) lines.push(`**Falhas do roteiro:** ${scenario.errors.join(" | ")}`);
  if (scenario?.lastAlert) lines.push(`**Alerta na tela:** ${scenario.lastAlert}`);
  const forced = iteration.forcedProcesses?.value ?? iteration.forcedProcesses;
  if (iteration.interruptedAt) {
    lines.push(
      `**Encerrada à força** ${seconds(Date.parse(iteration.interruptedAt) - launchedAt)} após o lançamento ` +
        `(${forced} processos): ${iteration.indexedPreviews} prévias no índice do Cache, ` +
        `${iteration.previewFiles} arquivos de prévia na pasta Media.`,
    );
  } else if (forced) lines.push(`Processos que o roteiro encerrou à força depois de fechar o projeto: ${forced}`);
  const jobs = events.filter((entry) => entry.event === "cache_job_completed");
  if (jobs.length) {
    const generated = jobs.filter((entry) => entry.outcome === "generated").length;
    lines.push(`Prévias no Processador de Imagens: ${generated} geradas, ${jobs.length - generated} reaproveitadas.`);
  }
  lines.push("");
  lines.push("| Etapa desde o lançamento | Tempo |");
  lines.push("|---|---|");
  for (const [label, name] of [
    ["Aplicativo iniciou", "application_started"],
    ["Host do Projeto pronto", "host_ready"],
    ["Projeto carregado", "project_load_completed"],
    ["Interface pronta", "project_ui_ready"],
    ["Janela do projeto apareceu", "project_window_presented"],
    ["Cena do canvas materializada", "canvas_scene_materialized"],
  ]) {
    lines.push(`| ${label} | ${seconds(first(name))} |`);
  }
  const stepAt = (name) => scenario?.steps?.find((step) => step.name === name)?.sinceLaunchMs;
  lines.push(`| Diálogo "Abrindo projeto" apareceu | ${seconds(stepAt("opening_dialog_seen"))} |`);
  lines.push(`| Diálogo "Abrindo projeto" fechou | ${seconds(stepAt("opening_dialog_closed"))} |`);
  lines.push(`| Canvas pronto para uso | ${seconds(stepAt("canvas_ready"))} |`);
  (scenario?.presentations ?? []).forEach((at, index) => {
    lines.push(`| Janela do projeto ${index + 1} apareceu | ${seconds(at)} |`);
  });
  lines.push(
    `| Última prévia de mídia pronta (${previews.length}/${count("media_preview_started")}) | ${seconds(
      previews.length ? previews.at(-1).at - launchedAt : null,
    )} |`,
  );
  lines.push("");
  // "Preparando imagens 34% 101 de 300" sampled from the opening dialog.
  const progress = (scenario?.openingProgress ?? [])
    .map((sample) => ({ ...sample, match: /(\d+) de (\d+)/.exec(sample.text) }))
    .filter((sample) => sample.match);
  if (progress.length >= 2) {
    const first = progress[0];
    const lastSample = progress.at(-1);
    const done = Number(lastSample.match[1]) - Number(first.match[1]);
    const elapsed = lastSample.sinceLaunchMs - first.sinceLaunchMs;
    lines.push(
      `Preparação de imagens na abertura: ${done} imagens em ${seconds(elapsed)} ` +
        `(${elapsed > 0 ? ((done * 60_000) / elapsed).toFixed(1).replace(".", ",") : "—"} por minuto), ` +
        `chegando a ${lastSample.match[1]} de ${lastSample.match[2]}.`,
    );
    const everyMinute = progress.filter(
      (sample, index) => index === 0 || index === progress.length - 1 || index % 4 === 0,
    );
    lines.push("");
    lines.push("| Desde o lançamento | Diálogo de abertura |");
    lines.push("|---|---|");
    for (const sample of everyMinute) lines.push(`| ${seconds(sample.sinceLaunchMs)} | ${sample.text} |`);
    lines.push("");
  }
  const memoryWaits = count("processor_memory_wait_started");
  if (memoryWaits) lines.push(`Esperas por memória do processador de imagens: ${memoryWaits}`);
  const problems = events.filter((entry) => entry.level === "ERROR" || entry.level === "WARN");
  if (problems.length) {
    const byEvent = Object.entries(
      problems.reduce((groups, entry) => {
        const key = `${entry.level} ${entry.event ?? entry.message ?? "?"}`;
        groups[key] = (groups[key] ?? 0) + 1;
        return groups;
      }, {}),
    );
    lines.push(`Avisos e erros do log: ${byEvent.map(([key, total]) => `${key} ×${total}`).join(", ")}`);
  }
  lines.push("");

  if (scenario?.phases?.length) {
    const frames = scenario.frames ?? [];
    const longTasks = scenario.longTasks ?? [];
    const animationFrames = scenario.animationFrames ?? [];
    const opening = taskStats(longTasks, 0, scenario.phases[0].start);
    lines.push(
      `Tarefas longas durante a abertura: ${opening.count}, somando ${ms(opening.total)}; a maior durou ${ms(opening.max)}.`,
    );
    lines.push("");
    lines.push("| Fase | Quadro p50 | Quadro p95 | Maior trava | Quadros > 50 ms | Tarefas longas | Resposta à roda p95 | Heap JS |");
    lines.push("|---|---|---|---|---|---|---|---|");
    for (const phase of scenario.phases) {
      const stats = frameStats(frames, phase.start, phase.end);
      const tasks = taskStats(longTasks, phase.start, phase.end);
      const loaf = taskStats(animationFrames, phase.start, phase.end);
      lines.push(
        `| ${phase.name} | ${ms(stats.p50)} | ${ms(stats.p95)} | ${ms(stats.max)} | ${stats.over50} | ` +
          `${tasks.count} (${ms(tasks.total)}; LoAF ${loaf.count}) | ${ms(percentile(phase.inputLatencies, 0.95))} | ` +
          `${mb((phase.metrics?.JSHeapUsedSize ?? 0) / 2 ** 20)} |`,
      );
    }
    lines.push("");
    for (const phase of scenario.phases) {
      const details = phase.details ?? {};
      const notes = [];
      if (details.error) notes.push(`**falhou:** ${details.error}`);
      if (details.navigations) {
        notes.push(
          "lâmina centralizada em " +
            details.navigations
              .map((entry) => `L${String(entry.sheet).padStart(2, "0")} ${entry.centredMs == null ? "não centralizou" : ms(entry.centredMs)}`)
              .join(", "),
        );
      }
      if ("suggestionsMs" in details) {
        notes.push(
          `${details.sheet ?? "painel"}: ${details.suggestions ?? 0} sugestões em ${details.suggestionsMs == null ? "mais de 30 s" : ms(details.suggestionsMs)}` +
            `${details.applied ? ", layout 2 aplicado" : ""}`,
        );
      }
      if ("dragSeen" in details) notes.push(details.dragSeen ? "o canvas reconheceu o arrasto" : "o canvas não reconheceu o arrasto");
      if ("saved" in details) notes.push(details.saved ? "projeto salvo" : "não confirmou que salvou");
      if (notes.length) lines.push(`- \`${phase.name}\`: ${notes.join("; ")}`);
    }
    lines.push("");
  }

  const samples = counters.filter((row) => row.timestamp >= launchedAt && row.timestamp < windowEnd);
  if (samples.length) {
    const max = (column) => Math.max(...samples.map((row) => row[column]));
    const min = (column) => Math.min(...samples.map((row) => row[column]));
    const mean = (column) => samples.reduce((sum, row) => sum + row[column], 0) / samples.length;
    lines.push("| Contador do convidado | Valor |");
    lines.push("|---|---|");
    lines.push(`| CPU total (pico / média) | ${pct(max("cpuPercent"))} / ${pct(mean("cpuPercent"))} |`);
    lines.push(`| Memória livre mínima | ${mb(min("availableMB"))} |`);
    lines.push(`| Memória comprometida máxima | ${mb(max("committedMB"))} |`);
    lines.push(`| Leituras de página do disco (pico) | ${Math.round(max("pagesInputPerSec"))}/s |`);
    lines.push(`| Disco ocupado (média) | ${pct(mean("diskPercent"))} |`);
    lines.push(
      `| Memória privada: app / WebView2 / imagens (pico) | ${mb(max("appPrivateMB"))} / ${mb(max("webviewPrivateMB"))} / ${mb(max("imagingPrivateMB"))} |`,
    );
    lines.push(
      `| CPU: app / WebView2 / imagens (pico) | ${pct(max("appCpuPercent"))} / ${pct(max("webviewCpuPercent"))} / ${pct(max("imagingCpuPercent"))} |`,
    );
    lines.push(`| Custo do amostrador (média) | ${pct(mean("samplerCpuPercent"))} |`);
    lines.push("");
  }

  writeUsage(lines, {
    scenario,
    roles: roles.filter((row) => row.timestamp >= launchedAt && row.timestamp < windowEnd),
    events,
    index: iteration.index,
  });
});

// Where the program spends, per iteration: process roles, JavaScript hot
// functions per phase, scripts of long animation frames and timed host work.
function writeUsage(lines, { scenario, roles: roleRows, events, index }) {
  const roleNames = {
    "app-project": "Host do Projeto (Rust)",
    "app-global": "Janela inicial / lançador (Rust)",
    "webview-renderer": "WebView2: páginas (JavaScript, layout)",
    "webview-gpu": "WebView2: GPU (desenho, texturas)",
    "webview-browser": "WebView2: processo principal",
    "webview-other": "WebView2: utilitários",
    imaging: "Processador de Imagens (Rust)",
    sampler: "Amostrador do laboratório",
  };
  const logical = run.guest?.logicalProcessors ?? 1;
  const usage = summarizeRoles(roleRows, logical);
  if (usage.length) {
    lines.push(`### Onde o programa gastou (abertura ${index})`);
    lines.push("");
    lines.push("| Parte | CPU (s) | Memória privada (pico) | Lido do disco | Gravado | Processos (pico) |");
    lines.push("|---|---|---|---|---|---|");
    for (const [role, entry] of usage) {
      lines.push(
        `| ${roleNames[role] ?? role} | ${entry.cpuSeconds.toFixed(1).replace(".", ",")} | ${mb(entry.peakPrivateMB)} | ` +
          `${mb(entry.readMB)} | ${mb(entry.writeMB)} | ${entry.peakProcesses} |`,
      );
    }
    lines.push("");
  }

  const timed = summarizeTimedEvents(events);
  if (timed.length) {
    lines.push("| Operação registrada pelo Host | Vezes | Total | p95 | Maior |");
    lines.push("|---|---|---|---|---|");
    for (const entry of timed) {
      lines.push(`| ${entry.key} | ${entry.count} | ${ms(entry.totalMs)} | ${ms(entry.p95)} | ${ms(entry.max)} |`);
    }
    lines.push("");
  }

  for (const profile of scenario?.profiles ?? []) {
    const path = join(runDirectory, profile.file);
    if (!existsSync(path)) continue;
    const analysis = analyzeCpuProfile(readJson(path), resolver);
    const busy = analysis.totals.sampledMs - analysis.totals.idleMs;
    lines.push(`#### JavaScript na fase \`${profile.name}\``);
    lines.push("");
    lines.push(
      `Ocupado ${ms(busy)} de ${ms(analysis.totals.sampledMs)} amostrados; ` +
        `coleta de lixo ${ms(analysis.totals.gcMs)}; trabalho nativo do navegador ${ms(analysis.totals.programMs)}.`,
    );
    lines.push("");
    if (analysis.functions.length) {
      lines.push("| Função (tempo próprio) | Onde | Tempo |");
      lines.push("|---|---|---|");
      for (const [key, value] of analysis.functions) {
        const [name, where] = key.split(" @ ");
        lines.push(`| \`${name}\` | ${where} | ${ms(value)} |`);
      }
      lines.push("");
      lines.push(`Por arquivo: ${analysis.files.map(([file, value]) => `${file} ${ms(value)}`).join("; ")}.`);
      lines.push("");
    }
  }

  const scripts = analyzeAnimationFrames(scenario?.animationFrames, resolver);
  if (scripts.length) {
    lines.push("Scripts nos quadros longos (LoAF):");
    lines.push("");
    for (const [key, entry] of scripts) lines.push(`- ${key}: ${entry.count}×, ${ms(entry.ms)}`);
    lines.push("");
  }
}

const summary = lines.join("\n");
writeFileSync(join(runDirectory, "summary.md"), summary);
console.log(summary);
