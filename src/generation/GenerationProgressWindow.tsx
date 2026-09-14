import { useEffect, useState } from "react";
import type { GenerationProgress, ProjectGenerationPort } from "../application/projectGeneration";
import { MessageDialog } from "../ui/MessageDialog";
import { OwnedWindowShell } from "../ui/OwnedWindowShell";
import { ProgressDialog } from "../ui/ProgressDialog";

export function GenerationProgressWindow({ port }: { port: ProjectGenerationPort }) {
  const [progress, setProgress] = useState<GenerationProgress | null>(null);
  const [failed, setFailed] = useState(false);
  const [cancelled, setCancelled] = useState(false);
  useEffect(() => {
    let active = true; let release: (() => void) | undefined; let received = false;
    void port.onProgress(next => { received = true; if (active) setProgress(next); }).then(dispose => { if (active) release = dispose; else dispose(); }).catch(() => { if (active) setFailed(true); });
    void port.progress().then(value => { if (active && !received) { if (value) setProgress(value); else setFailed(true); } }).catch(() => { if (active) setFailed(true); });
    return () => { active = false; release?.(); };
  }, [port]);
  const cancel = () => { setCancelled(true); void port.cancel().catch(() => setCancelled(false)); };
  if (!progress && !failed) return null;
  return <OwnedWindowShell width={400}>{failed ? <MessageDialog tone="error" title="Progresso indisponível" description="Cancele a geração e tente novamente." primaryAction={{ label: "Cancelar", disabled: cancelled, onClick: cancel }} /> : progress && <ProgressDialog title="Gerando Projetos" progress={{ kind: "determinate", completed: progress.completed, total: progress.total, countLabel: `${progress.completed} ${progress.completed === 1 ? "Projeto" : "Projetos"} de ${progress.total}` }} cancelAction={{ label: "Cancelar", disabled: cancelled, onClick: cancel }} />}</OwnedWindowShell>;
}
