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
    let active = true;
    let release: (() => void) | undefined;
    void port.onProgress(next => { if (active) setProgress(next); }).then(dispose => {
      if (active) release = dispose; else dispose();
    }).catch(() => { if (active) setFailed(true); });
    void port.progress().then(current => {
      if (active) { if (current) setProgress(current); else setFailed(true); }
    }).catch(() => { if (active) setFailed(true); });
    return () => { active = false; release?.(); };
  }, [port]);
  if (!progress && !failed) return null;
  if (failed) return <OwnedWindowShell controls="none" width={400}>
    <MessageDialog tone="error" title="Progresso indisponível" description="Cancele a exportação e tente novamente."
      primaryAction={{ label: "Cancelar", disabled: cancelled, onClick: () => {
        setCancelled(true); void port.cancel().catch(() => setCancelled(false));
      } }} />
  </OwnedWindowShell>;
  if (!progress) return null;
  return <OwnedWindowShell controls="none" width={400}>
    <ProgressDialog title="Exportando" progress={{ kind: "determinate", completed: progress.percent, total: 100,
      status: `${progress.completed}/${progress.total} Álbuns` }}
      cancelAction={{ label: "Cancelar", disabled: cancelled, onClick: () => {
        setCancelled(true);
        void port.cancel().catch(() => setCancelled(false));
      } }} />
  </OwnedWindowShell>;
}
