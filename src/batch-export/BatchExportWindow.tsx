import { useCallback, useEffect, useLayoutEffect, useState } from "react";
import type { BatchExportPort, BatchExportView, BatchRecoverySummary, ExportConflictPolicy } from "../application/batchExport";
import { ActionButton } from "../ui/ActionButton";
import { ConfirmationDialog } from "../ui/ConfirmationDialog";
import { MessageDialog } from "../ui/MessageDialog";
import { OwnedWindowShell } from "../ui/OwnedWindowShell";
import { ProblemsDialog } from "../ui/ProblemsDialog";
import { BatchConfiguration } from "./BatchConfiguration";

export function BatchExportWindow({ port }: { port: BatchExportPort }) {
  const [view, setView] = useState<BatchExportView | null>(null);
  const [recoveries, setRecoveries] = useState<BatchRecoverySummary[]>([]);
  const [busy, setBusy] = useState(false);
  const [conflicts, setConflicts] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const report = useCallback((reason: unknown) => setError(String(reason)), []);
  useEffect(() => {
    let active = true;
    let release: (() => void) | undefined;
    void port.onView(next => { if (active) setView(next); }).then(dispose => {
      if (active) release = dispose; else dispose();
    }).catch(report);
    void Promise.all([port.current(), port.recoveries()]).then(([current, pending]) => {
      if (active) { setView(current); setRecoveries(pending); }
    }).catch(report);
    return () => { active = false; release?.(); };
  }, [port, report]);
  useLayoutEffect(() => {
    if (view?.phase === "finished" || view?.phase === "interrupted") {
      void port.resultReady().catch(report);
    }
  }, [view, port, report]);
  const act = async (action: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    try { await action(); } catch (reason) { report(reason); } finally { setBusy(false); }
  };
  const close = () => void act(() => port.close());
  const run = async (policy: ExportConflictPolicy) => {
    setConflicts(false);
    setView(await port.run(policy));
  };
  const continueBatch = async (next: BatchExportView) => {
    if (next.hasConflicts) { setView(next); setConflicts(true); }
    else await run("ask");
  };
  const refresh = (operation: () => Promise<BatchExportView | null>) => void act(async () => {
    const next = await operation();
    if (next) setView(next); // Deliberately waits for the user's Continue action.
  });
  const end = (id: string) => void act(async () => {
    await port.end(id);
    setView(null);
    setRecoveries(await port.recoveries());
  });
  const recovery = !view ? recoveries[0] : undefined;
  const terminal = view?.phase === "finished" || view?.phase === "interrupted";
  const resultSummary = view ? `Exportados: ${view.items.filter(item => item.status === "completed").length} · Ignorados: ${view.items.filter(item => item.status === "ignored").length} · Com falha: ${view.items.filter(item => item.status === "failed").length}` : "";
  const problems = view?.items.filter(item => item.status !== "completed" &&
    (item.problems.length > 0 || item.status === "ignored")) ?? [];
  let content;
  if (conflicts && view) {
    content = <ConfirmationDialog title="Já existe uma exportação" tone="neutral"
      description="Como deseja tratar os arquivos existentes?"
      leadingAction={{ label: "Ignorar", disabled: busy, onClick: () => void act(() => run("skip")) }}
      cancelAction={{ label: "Cancelar", disabled: busy, onClick: () => setConflicts(false) }}
      confirmAction={{ label: "Substituir", disabled: busy, onClick: () => void act(() => run("replace")) }} />;
  } else if (recovery) {
    content = <ConfirmationDialog title="Lote interrompido" description={<>
      {recovery.remaining} de {recovery.total} Projetos para concluir.
      <span className="batch-project-path" title={recovery.sourceFolder}>{recovery.sourceFolder}</span>
    </>} cancelAction={{ label: "Encerrar", disabled: busy, onClick: () => end(recovery.id) }}
      confirmAction={{ label: "Retomar", disabled: busy, onClick: () => refresh(() => port.resume(recovery.id)) }} />;
  } else if (view?.phase === "interrupted") {
    content = <ConfirmationDialog title="Lote interrompido" description="Os arquivos já exportados foram mantidos."
      cancelAction={{ label: "Encerrar", disabled: busy, onClick: () => end(view.id) }}
      confirmAction={{ label: "Retomar", disabled: busy, onClick: () => refresh(() => port.resume(view.id)) }} />;
  } else if (terminal && problems.length === 0) {
    content = <MessageDialog title="Exportação concluída" description="Todos os Álbuns foram exportados." tone="success"
      primaryAction={{ label: "Fechar", disabled: busy, onClick: close }} />;
  } else if (view && problems.length > 0) {
    content = <ProblemsDialog title={terminal ? "Resultado da exportação" : "Problemas na exportação"}
      description={terminal ? resultSummary : "Resolva ou ignore os Projetos abaixo para continuar."}
      columns={["Projeto", "Problema", "Ações"]} rows={problems.map(item => [
        <span key="project">{item.name}<span className="batch-project-path" title={item.projectPath}>{item.projectPath}</span></span>,
        <div key="reasons" className="batch-problem-reasons">{item.status === "ignored" && <strong>Ignorado neste lote</strong>}
          {item.problems.map((problem, index) => <span key={index}>{problem.message}</span>)}
        </div>,
        !terminal && item.status !== "ignored" ? <div key="actions" className="batch-problem-actions">
          <ActionButton disabled={busy} onClick={() => void act(async () => {
            const outcome = await port.openProject(item.id);
            if (outcome.status === "failed") throw new Error(outcome.error.message);
          })}>Abrir Projeto</ActionButton>
          {item.problems.some(problem => problem.kind === "missingMedia") && <ActionButton disabled={busy}
            onClick={() => refresh(() => port.relink(item.id))}>Religar…</ActionButton>}
          <ActionButton disabled={busy} onClick={() => refresh(() => port.ignore(item.id))}>Ignorar neste lote</ActionButton>
        </div> : "—",
      ])} closeDisabled={busy} onClose={close} actions={terminal ? <>
        {view.items.some(item => item.status === "failed") && <ActionButton disabled={busy}
          onClick={() => refresh(() => port.resume(view.id))}>Tentar novamente</ActionButton>}
        <ActionButton disabled={busy} onClick={() => end(view.id)}>Encerrar</ActionButton>
      </> : <>
        {problems.some(item => item.status !== "ignored" && item.problems.some(problem => problem.kind === "missingMedia")) &&
          <ActionButton disabled={busy} onClick={() => refresh(() => port.relink(null))}>Religar todos…</ActionButton>}
        <ActionButton disabled={busy} onClick={() => refresh(() => port.recheck())}>Tentar novamente</ActionButton>
        <ActionButton variant="primary" disabled={busy || !view.canContinue}
          onClick={() => void act(() => continueBatch(view))}>Continuar Exportação</ActionButton>
      </>} />;
  } else if (view) {
    content = <ConfirmationDialog title="Pronto para exportar" tone="neutral"
      description={`${view.items.filter(item => item.status === "pending").length} Projetos prontos.`}
      cancelAction={{ label: "Cancelar", disabled: busy, onClick: close }}
      confirmAction={{ label: "Continuar Exportação", disabled: busy || !view.canContinue,
        onClick: () => void act(() => continueBatch(view)) }} />;
  }
  return <OwnedWindowShell controls={busy ? "none" : "close"} context="Exportação em lote" width={800}>
    <div className="batch-export" hidden={error !== null}>
      {!view && !recovery && !conflicts ? <BatchConfiguration port={port} busy={busy} onError={report} onClose={close}
        onSubmit={options => void act(async () => {
          const next = await port.prepare(options);
          if (next.canContinue) await continueBatch(next);
          else setView(next);
        })} /> : content}
    </div>
    {error !== null && <MessageDialog tone="error" title="Não foi possível concluir" description={error}
      primaryAction={{ label: "Voltar", onClick: () => setError(null) }} />}
  </OwnedWindowShell>;
}
