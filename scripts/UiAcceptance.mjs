import path from "node:path";

const scenarioIdPattern = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;
const elementKey = "element-6066-11e4-a52e-4f735466cecf";

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
  invariant(manifest.schemaVersion === 1, "schemaVersion must be 1");
  invariant(Array.isArray(manifest.scenarios) && manifest.scenarios.length > 0, "scenarios must be a non-empty array");

  const ids = new Set();
  for (const [index, scenario] of manifest.scenarios.entries()) {
    const location = `scenarios[${index}]`;
    invariant(scenario && typeof scenario === "object", `${location} must be an object`);
    invariant(typeof scenario.id === "string" && scenarioIdPattern.test(scenario.id), `${location}.id must be kebab-case`);
    invariant(!ids.has(scenario.id), `${location}.id duplicates ${scenario.id}`);
    ids.add(scenario.id);
    invariant(typeof scenario.title === "string" && scenario.title.trim(), `${location}.title is required`);
    validateServedPath(scenario.implementationPath, `${location}.implementationPath`);
    validateServedPath(scenario.referencePath, `${location}.referencePath`);
    invariant(scenario.viewport && typeof scenario.viewport === "object", `${location}.viewport is required`);
    for (const dimension of ["width", "height"]) {
      const value = scenario.viewport[dimension];
      invariant(Number.isInteger(value) && value >= 320 && value <= 4096, `${location}.viewport.${dimension} must be an integer between 320 and 4096`);
    }
    invariant(typeof scenario.readySelector === "string" && scenario.readySelector.trim(), `${location}.readySelector is required`);
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
          action.type === "click" || action.type === "click-text",
          `${actionLocation}.type must be click or click-text`,
        );
        if (action.type === "click") {
          invariant(typeof action.selector === "string" && action.selector.trim(), `${actionLocation}.selector is required`);
        } else {
          invariant(typeof action.text === "string" && action.text.trim(), `${actionLocation}.text is required`);
        }
      }
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
  const id = element?.[elementKey];
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
  const screenshots = passed
    ? `<div class="comparison">
        <figure><figcaption>Implementação</figcaption><img src="${escapeHtml(scenario.implementationScreenshot)}" alt="Captura da implementação — ${escapeHtml(scenario.title)}"></figure>
        <figure><figcaption>Referência vigente</figcaption><img src="${escapeHtml(scenario.referenceScreenshot)}" alt="Captura da referência — ${escapeHtml(scenario.title)}"></figure>
      </div>`
    : `<pre class="failure">${escapeHtml(scenario.error ?? "Falha sem mensagem")}</pre>`;

  return `<article class="scenario ${passed ? "scenario--captured" : "scenario--failed"}">
    <header>
      <div><p class="scenario-id">${escapeHtml(scenario.id)}</p><h2>${escapeHtml(scenario.title)}</h2></div>
      <span class="status">${passed ? "Capturado · não revisado" : "Captura falhou"}</span>
    </header>
    <p class="meta">${escapeHtml(`${scenario.viewport.width} × ${scenario.viewport.height}`)} · <a href="${escapeHtml(scenario.implementationUrl)}">implementação</a> · <a href="${escapeHtml(scenario.referenceUrl)}">referência</a></p>
    ${screenshots}
  </article>`;
}

export function renderUiAcceptanceReport(evidence) {
  const scenarioHtml = evidence.scenarios.map(scenarioCard).join("\n");
  const captured = evidence.scenarios.filter((scenario) => scenario.captureStatus === "captured-unreviewed").length;
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
      <p class="summary">${escapeHtml(captured)} de ${escapeHtml(evidence.scenarios.length)} cenários foram capturados; ${escapeHtml(failed)} falharam. Commit: <code>${escapeHtml(evidence.gitCommit)}</code>${evidence.sourceInputsDirty ? " · árvore com alterações locais" : " · árvore limpa"}.</p>
    </header>
    <div class="notice"><strong>Nenhuma captura foi aprovada automaticamente.</strong> O estado “capturado” confirma somente que a evidência reproduzível foi produzida. Uma pessoa ainda deve comparar cada par e registrar o resultado da revisão.</div>
    ${scenarioHtml}
    <footer>Gerado em ${escapeHtml(evidence.collectedAtUtc)} · status global: ${escapeHtml(evidence.captureStatus)}</footer>
  </main>
</body>
</html>`;
}
