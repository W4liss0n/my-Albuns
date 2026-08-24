import React from "react";
import ReactDOM from "react-dom/client";

import manifest from "./test/uiAcceptanceScenarios.json";
import "./App.css";
import "./ui-acceptance-preview.css";

type PreviewScenario = (typeof manifest.scenarios)[number];

function absoluteUrl(path: string): string {
  return new URL(path, window.location.origin).toString();
}

function ScenarioCard({ scenario }: { scenario: PreviewScenario }) {
  return (
    <article className="acceptance-scenario">
      <div>
        <h2>{scenario.title}</h2>
        <p>{scenario.id}</p>
      </div>
      <dl>
        <div>
          <dt>Viewport</dt>
          <dd>
            {scenario.viewport.width} × {scenario.viewport.height}
          </dd>
        </div>
        <div>
          <dt>Estado pronto</dt>
          <dd>{scenario.readySelector}</dd>
        </div>
      </dl>
      <div className="acceptance-scenario__actions">
        <a href={absoluteUrl(scenario.implementationPath)}>Implementação</a>
        <a href={absoluteUrl(scenario.referencePath)}>Referência vigente</a>
      </div>
    </article>
  );
}

function UiAcceptancePreview() {
  return (
    <main className="acceptance-index">
      <header>
        <p className="acceptance-index__eyebrow">MYALBUNS · DESENVOLVIMENTO</p>
        <h1>Cenários de aceitação da UI</h1>
        <p>
          Esta página apenas navega pelas fontes declaradas. A evidência
          reproduzível, com screenshots e logs, é produzida por
          <code> npm run ui:acceptance</code>.
        </p>
      </header>
      <section aria-label="Cenários visuais" className="acceptance-index__grid">
        {manifest.scenarios.map((scenario) => (
          <ScenarioCard key={scenario.id} scenario={scenario} />
        ))}
      </section>
    </main>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <UiAcceptancePreview />
  </React.StrictMode>,
);
