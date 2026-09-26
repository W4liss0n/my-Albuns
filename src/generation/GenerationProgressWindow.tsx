import { observeSnapshot } from "../application/observeSnapshot";
import { useEffect, useState } from "react";
import type { GenerationProgress, ProjectGenerationPort } from "../application/projectGeneration";
import { MessageDialog } from "../ui/MessageDialog";
import { OwnedWindowShell } from "../ui/OwnedWindowShell";
import { ProgressDialog } from "../ui/ProgressDialog";

export function GenerationProgressWindow({ port }: { port: Pick<ProjectGenerationPort, "progress" | "onProgress" | "cancel"> }) {
  const [progress, setProgress] = useState<GenerationProgress | null>(null);
  const [failed, setFailed] = useState(false);
  const [cancelled, setCancelled] = useState(false);
  useEffect(() => {
    const observation = observeSnapshot({
      subscribe: receive => port.onProgress(receive), read: () => port.progress(),
      receive: value => { if (value) setProgress(value); else setFailed(true); },
    });
    void observation.ready.catch(() => setFailed(true));
    return observation.dispose;
  }, [port]);
  const cancel = () => { setCancelled(true); void port.cancel().catch(() => setCancelled(false)); };
  return <OwnedWindowShell context="Gerar projetos em lote" width={400}>{failed ? <MessageDialog tone="error" title="Progresso indisponível" description="Cancele a geração e tente novamente." primaryAction={{ label: "Cancelar", disabled: cancelled, onClick: cancel }} /> : <ProgressDialog
    title="Gerando projetos"
    reserveProgressMeta
    progress={progress?.total == null ? { kind: "indeterminate", status: null } : {
      kind: "determinate", completed: progress.completed, total: progress.total,
      countLabel: `${progress.completed} ${progress.completed === 1 ? "projeto" : "projetos"} de ${progress.total}`,
    }}
    cancelAction={{ label: "Cancelar", disabled: cancelled, onClick: cancel }}
  />}</OwnedWindowShell>;
}
