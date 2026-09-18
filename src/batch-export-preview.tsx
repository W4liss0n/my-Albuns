import ReactDOM from "react-dom/client";
import type { BatchExportPort, BatchExportView } from "./application/batchExport";
import { BatchExportWindow } from "./batch-export/BatchExportWindow";
import { BatchProgressWindow } from "./batch-export/BatchProgressWindow";
import "./ui/theme.css";
import "./ui/ui.css";
import "./batch-export/batchExport.css";

const scenario = new URLSearchParams(window.location.search).get("scenario");
const ready: BatchExportView = {
  id: "preview", phase: "prepared", canContinue: true, hasConflicts: scenario === "conflicts", partialPublication: false,
  options: { sourceFolder: "D:\\Projetos\\Formatura 2026", destinationFolder: null, format: { kind: "jpeg", quality: 100 }, mode: "sheet" },
  items: ["Ana Oliveira", "Pedro Santos", "Turma 3 — Escola São José"].map((name, index) => ({
    id: String(index), name, projectPath: `D:\\Projetos\\Formatura 2026\\${name}.myalbuns`,
    destination: `D:\\Projetos\\Formatura 2026\\${name}`, status: "pending", problems: [],
  })),
};
const view: BatchExportView | null = scenario === "configuration" || !scenario || scenario === "recovery" ? null
  : scenario === "problems" ? { ...ready, canContinue: false, items: ready.items.map((item, index) => ({ ...item,
    problems: index === 0 ? [{ kind: "missingMedia", mediaId: "photo", message: "Imagem ausente: 001.jpg" }]
      : index === 1 ? [{ kind: "placeholder", mediaId: null, message: "Preencha os quadros vazios e salve o projeto." }]
      : [{ kind: "unavailable", mediaId: null, message: "Projeto indisponível. Verifique a pasta e tente novamente." }],
  })) }
  : scenario === "result" ? { ...ready, phase: "finished", items: ready.items.map((item, index) => ({ ...item,
    status: index === 0 ? "completed" : index === 1 ? "ignored" : "failed",
    problems: index === 2 ? [{ kind: "failed", mediaId: null, message: "O projeto mudou depois da verificação. Verifique novamente antes de exportar." }] : [],
  })) } : scenario.startsWith("storage-full") ? { ...ready, phase: "storageFull", canContinue: false, partialPublication: scenario === "storage-full-partial",
    items: ready.items.map((item, index) => ({ ...item, status: index === 0 ? "completed" : "pending" })) }
  : scenario === "success" ? { ...ready, phase: "finished", items: ready.items.map(item => ({ ...item, status: "completed" })) } : ready;
const port: BatchExportPort = {
  storageRecovery: { status: async () => ({ id: "pause", canClearCache: scenario === "storage-full-cache" }), clear: async () => false },
  current: async () => view,
  recoveries: async () => scenario === "recovery" ? [{ id: "preview", sourceFolder: ready.options.sourceFolder, total: 18, remaining: 7 }] : [],
  chooseFolder: async () => ready.options.sourceFolder, countProjects: async () => 18,
  prepare: async () => ready, recheck: async () => ready, ignore: async () => ready, relink: async () => ready,
  openProject: async () => ({ status: "opened" }), run: async () => ready, cancel: async () => undefined,
  resume: async () => ready, end: async () => undefined, close: async () => undefined,
  resultReady: async () => undefined, progress: async () => ({ completed: 7, total: 18, percent: 43 }),
  onView: async () => () => undefined, onProgress: async () => () => undefined,
};
ReactDOM.createRoot(document.getElementById("root")!).render(scenario === "progress"
  ? <BatchProgressWindow port={port} /> : <BatchExportWindow port={port} />);
