import path from "node:path";

const scenarioIdPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const surfaceIdPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
export const webdriverElementKey = "element-6066-11e4-a52e-4f735466cecf";
const supportedKeys = new Set([
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "ArrowUp",
  "Enter",
  "Escape",
  "Space",
  "Tab",
]);

function invariant(condition, message) {
  if (!condition) throw new Error(`Invalid UI acceptance manifest: ${message}`);
}

function validateServedPath(value, label) {
  invariant(typeof value === "string" && value.startsWith("/"), `${label} must be a root-relative path`);
  invariant(!value.startsWith("//"), `${label} must not be a protocol-relative URL`);
  const rawPathname = decodeURIComponent(value.split(/[?#]/u, 1)[0]);
  invariant(!rawPathname.split("/").includes(".."), `${label} must not escape the workspace`);
}

export function validateUiAcceptanceManifest(manifest) {
  invariant(manifest && typeof manifest === "object", "the document must be an object");
  invariant(manifest.schemaVersion === 2, "schemaVersion must be 2");
  invariant(Array.isArray(manifest.scenarios) && manifest.scenarios.length > 0, "scenarios must be a non-empty array");

  const ids = new Set();
  const pairedReferenceStates = new Set();
  for (const [index, scenario] of manifest.scenarios.entries()) {
    const location = `scenarios[${index}]`;
    invariant(scenario && typeof scenario === "object", `${location} must be an object`);
    invariant(typeof scenario.id === "string" && scenarioIdPattern.test(scenario.id), `${location}.id must be kebab-case`);
    invariant(!ids.has(scenario.id), `${location}.id duplicates ${scenario.id}`);
    ids.add(scenario.id);
    invariant(typeof scenario.title === "string" && scenario.title.trim(), `${location}.title is required`);
    validateServedPath(scenario.implementationPath, `${location}.implementationPath`);
    invariant(
      scenario.comparison && typeof scenario.comparison === "object",
      `${location}.comparison is required`,
    );
    invariant(
      scenario.comparison.kind === "paired" ||
        scenario.comparison.kind === "implementation-only",
      `${location}.comparison.kind must be paired or implementation-only`,
    );
    invariant(
      typeof scenario.comparison.surface === "string" &&
        surfaceIdPattern.test(scenario.comparison.surface),
      `${location}.comparison.surface must be kebab-case`,
    );
    const implementationCaptureSelector =
      scenario.comparison.implementationCaptureSelector;
    invariant(
      implementationCaptureSelector === undefined ||
        (typeof implementationCaptureSelector === "string" &&
          implementationCaptureSelector.trim()),
      `${location}.comparison.implementationCaptureSelector must be a non-empty string when present`,
    );
    if (scenario.comparison.kind === "paired") {
      validateServedPath(scenario.referencePath, `${location}.referencePath`);
      invariant(
        scenario.comparison.reason === undefined,
        `${location}.comparison.reason is only valid for implementation-only scenarios`,
      );
      const referenceCaptureSelector =
        scenario.comparison.referenceCaptureSelector;
      invariant(
        referenceCaptureSelector === undefined ||
          (typeof referenceCaptureSelector === "string" &&
            referenceCaptureSelector.trim()),
        `${location}.comparison.referenceCaptureSelector must be a non-empty string when present`,
      );
      invariant(
        Boolean(implementationCaptureSelector) ===
          Boolean(referenceCaptureSelector),
        `${location}.comparison capture selectors must be provided for both sides or neither side`,
      );
    } else {
      invariant(
        scenario.referencePath === undefined,
        `${location}.referencePath must be omitted for implementation-only scenarios`,
      );
      invariant(
        scenario.referenceActions === undefined,
        `${location}.referenceActions must be omitted for implementation-only scenarios`,
      );
      invariant(
        scenario.referenceReadySelector === undefined,
        `${location}.referenceReadySelector must be omitted for implementation-only scenarios`,
      );
      invariant(
        scenario.comparison.referenceCaptureSelector === undefined,
        `${location}.comparison.referenceCaptureSelector must be omitted for implementation-only scenarios`,
      );
      invariant(
        typeof scenario.comparison.reason === "string" &&
          scenario.comparison.reason.trim(),
        `${location}.comparison.reason is required for implementation-only scenarios`,
      );
    }
    invariant(scenario.viewport && typeof scenario.viewport === "object", `${location}.viewport is required`);
    for (const dimension of ["width", "height"]) {
      const value = scenario.viewport[dimension];
      invariant(Number.isInteger(value) && value >= 320 && value <= 4096, `${location}.viewport.${dimension} must be an integer between 320 and 4096`);
    }
    invariant(typeof scenario.readySelector === "string" && scenario.readySelector.trim(), `${location}.readySelector is required`);
    invariant(
      scenario.referenceReadySelector === undefined ||
        (typeof scenario.referenceReadySelector === "string" &&
          scenario.referenceReadySelector.trim()),
      `${location}.referenceReadySelector must be a non-empty string when present`,
    );
    invariant(Array.isArray(scenario.actions), `${location}.actions must be an array`);
    const actionGroups = [
      ["actions", scenario.actions],
      ["referenceActions", scenario.referenceActions ?? []],
    ];
    invariant(
      scenario.referenceActions === undefined ||
        Array.isArray(scenario.referenceActions),
      `${location}.referenceActions must be an array when present`,
    );
    for (const [groupName, actions] of actionGroups) {
      for (const [actionIndex, action] of actions.entries()) {
        const actionLocation = `${location}.${groupName}[${actionIndex}]`;
        invariant(action && typeof action === "object", `${actionLocation} must be an object`);
        invariant(
          ["click", "click-text", "focus", "hover", "input", "key"].includes(action.type),
          `${actionLocation}.type is not supported`,
        );
        if (["click", "focus", "hover", "input"].includes(action.type)) {
          invariant(typeof action.selector === "string" && action.selector.trim(), `${actionLocation}.selector is required`);
        }
        if (action.type === "click-text") {
          invariant(typeof action.text === "string" && action.text.trim(), `${actionLocation}.text is required`);
        }
        if (action.type === "input") {
          invariant(typeof action.value === "string", `${actionLocation}.value must be a string`);
        }
        if (action.type === "key") {
          invariant(supportedKeys.has(action.key), `${actionLocation}.key is not supported`);
          invariant(action.selector === undefined, `${actionLocation}.selector is not valid for key actions`);
        }
      }
    }
    if (scenario.comparison.kind === "paired") {
      const referenceState = JSON.stringify({
        actions: scenario.referenceActions ?? [],
        captureSelector: scenario.comparison.referenceCaptureSelector ?? null,
        path: scenario.referencePath,
        viewport: scenario.viewport,
      });
      invariant(
        !pairedReferenceStates.has(referenceState),
        `${location} duplicates a reference state captured by another scenario`,
      );
      pairedReferenceStates.add(referenceState);
    }
  }
  return manifest;
}

export function servedFilePath(workspace, servedPath) {
  validateServedPath(servedPath, "servedPath");
  const pathname = decodeURIComponent(new URL(servedPath, "http://127.0.0.1").pathname);
  const candidate = path.resolve(workspace, pathname.slice(1));
  const relative = path.relative(path.resolve(workspace), candidate);
  invariant(relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative), "servedPath must remain inside the workspace");
  return candidate;
}

export function webdriverElementId(element) {
  const id = element?.[webdriverElementKey];
  if (typeof id !== "string" || !id) {
    throw new Error("WebDriver returned an invalid element reference");
  }
  return id;
}

export function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function scenarioCard(scenario) {
  const passed = scenario.captureStatus === "captured-unreviewed";
  const paired = scenario.comparison.kind === "paired";
  const screenshots = passed
    ? paired
      ? `<div class="comparison">
        <figure><figcaption>Implementação</figcaption><img src="${escapeHtml(scenario.implementationScreenshot)}" alt="Captura da implementação — ${escapeHtml(scenario.title)}"></figure>
        <figure><figcaption>Referência vigente</figcaption><img src="${escapeHtml(scenario.referenceScreenshot)}" alt="Captura da referência — ${escapeHtml(scenario.title)}"></figure>
      </div>`
      : `<div class="implementation-only">
        <figure><figcaption>Implementação</figcaption><img src="${escapeHtml(scenario.implementationScreenshot)}" alt="Captura da implementação — ${escapeHtml(scenario.title)}"></figure>
        <p class="unvalidated"><strong>Sem referência visual equivalente.</strong> ${escapeHtml(scenario.comparison.reason)}</p>
      </div>`
    : `<pre class="failure">${escapeHtml(scenario.error ?? "Falha sem mensagem")}</pre>`;
  const status = passed
    ? paired
      ? "Capturado · não revisado"
      : "Implementação capturada · não revisada"
    : "Captura falhou";
  const referenceMeta = paired
    ? ` · <a href="${escapeHtml(scenario.referenceUrl)}">referência</a>`
    : " · sem referência equivalente";

  return `<article class="scenario ${passed ? "scenario--captured" : "scenario--failed"}">
    <header>
      <div><p class="scenario-id">${escapeHtml(scenario.id)}</p><h2>${escapeHtml(scenario.title)}</h2></div>
      <span class="status">${status}</span>
    </header>
    <p class="meta">${escapeHtml(`${scenario.viewport.width} × ${scenario.viewport.height}`)} · superfície: <code>${escapeHtml(scenario.comparison.surface)}</code> · <a href="${escapeHtml(scenario.implementationUrl)}">implementação</a>${referenceMeta}</p>
    ${screenshots}
  </article>`;
}

export function renderUiAcceptanceReport(evidence) {
  const scenarioHtml = evidence.scenarios.map(scenarioCard).join("\n");
  const captured = evidence.scenarios.filter((scenario) => scenario.captureStatus === "captured-unreviewed").length;
  const capturedPaired = evidence.scenarios.filter(
    (scenario) =>
      scenario.captureStatus === "captured-unreviewed" &&
      scenario.comparison.kind === "paired",
  ).length;
  const capturedImplementationOnly = captured - capturedPaired;
  const failed = evidence.scenarios.length - captured;
  return `<!doctype html>
<html lang="pt-BR">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Aceitação visual · MyAlbuns</title>
  <style>
    :root { color-scheme: light; font-family: "Segoe UI", sans-serif; color: #2d2a26; background: #f3f1ed; }
    * { box-sizing: border-box; }
    body { margin: 0; }
    main { width: min(1680px, calc(100% - 48px)); margin: 0 auto; padding: 44px 0 72px; }
    .eyebrow, .scenario-id { margin: 0 0 6px; color: #8b7667; font: 600 11px/1.2 Consolas, monospace; letter-spacing: .12em; text-transform: uppercase; }
    h1 { margin: 0; font-size: 30px; font-weight: 650; }
    .summary { max-width: 820px; color: #6e6259; line-height: 1.55; }
    .notice { margin: 22px 0 30px; padding: 14px 16px; border-left: 3px solid #b6805c; background: #fffaf6; color: #654b3a; }
    .scenario { margin: 0 0 28px; padding: 20px; border: 1px solid #ddd5cc; background: #fff; box-shadow: 0 7px 22px rgb(66 52 39 / 7%); }
    .scenario > header { display: flex; align-items: start; justify-content: space-between; gap: 20px; }
    h2 { margin: 0; font-size: 18px; font-weight: 600; }
    .status { padding: 5px 9px; border-radius: 999px; background: #eee9e3; color: #6d5f54; font-size: 12px; white-space: nowrap; }
    .scenario--failed .status { background: #fae7e4; color: #9b3e36; }
    .meta { margin: 12px 0 18px; color: #857970; font: 12px/1.4 Consolas, monospace; }
    a { color: #356e98; }
    .comparison { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; align-items: start; }
    .implementation-only { display: grid; gap: 14px; }
    .unvalidated { margin: 0; padding: 12px 14px; border-left: 3px solid #b6805c; background: #fffaf6; color: #654b3a; line-height: 1.5; }
    figure { margin: 0; min-width: 0; }
    figcaption { margin-bottom: 7px; color: #6d6259; font-size: 12px; font-weight: 600; }
    img { display: block; width: 100%; height: auto; border: 1px solid #d8d1c8; background: #faf9f7; }
    .failure { padding: 14px; overflow: auto; border: 1px solid #e9bcb6; background: #fff5f3; color: #7a2e29; white-space: pre-wrap; }
    footer { color: #857970; font-size: 12px; }
    @media (max-width: 900px) { .comparison { grid-template-columns: 1fr; } main { width: min(100% - 24px, 1680px); padding-top: 24px; } }
  </style>
</head>
<body>
  <main>
    <header>
      <p class="eyebrow">MYALBUNS · EVIDÊNCIA DE DESENVOLVIMENTO</p>
      <h1>Aceitação visual</h1>
      <p class="summary">${escapeHtml(captured)} de ${escapeHtml(evidence.scenarios.length)} cenários foram capturados (${escapeHtml(capturedPaired)} pares e ${escapeHtml(capturedImplementationOnly)} somente da implementação); ${escapeHtml(failed)} falharam. Commit: <code>${escapeHtml(evidence.gitCommit)}</code>${evidence.sourceInputsDirty ? " · árvore com alterações locais" : " · árvore limpa"}.</p>
    </header>
    <div class="notice"><strong>Nenhuma captura foi aprovada automaticamente.</strong> O estado “capturado” confirma somente que a evidência reproduzível foi produzida. Uma pessoa ainda deve comparar cada par e registrar o resultado da revisão.</div>
    ${scenarioHtml}
    <footer>Gerado em ${escapeHtml(evidence.collectedAtUtc)} · status global: ${escapeHtml(evidence.captureStatus)}</footer>
  </main>
</body>
</html>`;
}
