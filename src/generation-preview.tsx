import ReactDOM from "react-dom/client";
import type { GenerationView, ProjectGenerationPort } from "./application/projectGeneration";
import { GenerationWindow } from "./generation/GenerationWindow";
import { GenerationProgressWindow } from "./generation/GenerationProgressWindow";
import "./ui/theme.css";
import "./ui/ui.css";

const scenario = new URLSearchParams(window.location.search).get("scenario");
const base: GenerationView = { id: "preview", phase: "prepared", canContinue: false,
  options: { sourceFolder: "D:\\Fotos\\Formatura 2026", destinationFolder: "E:\\Álbuns\\Formatura 2026" },
  items: ["Ana Oliveira", "Pedro Santos", "Turma 3 — Escola São José"].map((name, index) => ({
    id: String(index), name, destination: `E:\\Álbuns\\Formatura 2026\\${name}.myalbuns`, status: "pending", decision: null,
    conflict: true, canReplace: index === 0, problems: index === 1 ? ["O Projeto está aberto. Feche-o e verifique novamente, ou ignore este item."] : index === 2 ? ["foto_001.jpg: Não foi possível ler a imagem."] : [],
  })),
};
const view: GenerationView | null = scenario === "configuration" ? null : scenario === "ready" ? { ...base, canContinue: true, items: base.items.map(item => ({ ...item, conflict: false, problems: [] })) }
  : scenario === "result" ? { ...base, phase: "finished", items: base.items.map((item, index) => ({ ...item, status: index === 0 ? "completed" : index === 1 ? "ignored" : "failed" })) }
  : scenario === "success" ? { ...base, phase: "finished", items: base.items.map(item => ({ ...item, status: "completed", problems: [] })) }
  : scenario === "cancelled" ? { ...base, phase: "cancelled", items: base.items.map((item, index) => ({ ...item, status: index === 0 ? "completed" : "pending", problems: [] })) } : base;
const port: ProjectGenerationPort = {
  model: async () => "Modelo Formatura 2026", chooseFolder: async () => null, count: async () => 18, current: async () => view,
  prepare: async () => base, decide: async () => base, recheck: async () => base, run: async () => base,
  progress: async () => scenario === "preparing" ? ({ completed: 0, total: null }) : ({ completed: 7, total: 18 }), onView: async () => () => {}, onProgress: async () => () => {}, resultReady: async () => {}, cancel: async () => {}, close: async () => {},
};
ReactDOM.createRoot(document.getElementById("root")!).render(scenario === "progress" || scenario === "preparing" ? <GenerationProgressWindow port={port} /> : <GenerationWindow port={port} />);
