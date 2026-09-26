import { observeSnapshot } from "../application/observeSnapshot";
import { useEffect, useState } from "react";
import type { BatchExportPort, BatchExportProgress } from "../application/batchExport";
import { OwnedWindowShell } from "../ui/OwnedWindowShell";
import { ProgressDialog } from "../ui/ProgressDialog";
import { MessageDialog } from "../ui/MessageDialog";

export function BatchProgressWindow({ port }: { port: BatchExportPort }) {
  const [progress, setProgress] = useState<BatchExportProgress | null>(null);
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
  if (!progress && !failed) return null;
  if (failed) return <OwnedWindowShell context="Exportação em lote" controls="none" width={400}>
    <MessageDialog tone="error" title="Progresso indisponível" description="Cancele a exportação e tente novamente."
      primaryAction={{ label: "Cancelar", disabled: cancelled, onClick: () => {
        setCancelled(true); void port.cancel().catch(() => setCancelled(false));
      } }} />
  </OwnedWindowShell>;
  if (!progress) return null;
  return <OwnedWindowShell context="Exportação em lote" controls="none" width={400}>
    <ProgressDialog title="Exportando" progress={{ kind: "determinate", completed: progress.percent, total: 100,
      status: null,
      countLabel: `${progress.completed} ${progress.completed === 1 ? "álbum" : "álbuns"} de ${progress.total}` }}
      cancelAction={{ label: "Cancelar", disabled: cancelled, onClick: () => {
        setCancelled(true);
        void port.cancel().catch(() => setCancelled(false));
      } }} />
  </OwnedWindowShell>;
}
