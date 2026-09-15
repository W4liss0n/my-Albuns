import { useCallback, useEffect, useState } from "react";
import { PanelsTopLeft } from "lucide-react";
import type { GenerationOptions, GenerationView, ProjectGenerationPort } from "../application/projectGeneration";
import { ActionButton } from "../ui/ActionButton";
import { AppIcon } from "../ui/AppIcon";
import { DialogWindowFrame } from "../ui/DialogWindowFrame";
import { MessageDialog } from "../ui/MessageDialog";
import { OwnedWindowShell } from "../ui/OwnedWindowShell";
import { ProblemsDialog } from "../ui/ProblemsDialog";
import { TextInput } from "../ui/TextInput";
import "../ui/OperationForm.css";
import "./generation.css";

export function GenerationWindow({ port }: { port: ProjectGenerationPort }) {
  const [model, setModel] = useState("");
  const [view, setView] = useState<GenerationView | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const report = useCallback((error: unknown) => setError(String(error)), []);
  useEffect(() => {
    let active = true;
    let receivedView = false;
    let release: (() => void) | undefined;
    void Promise.all([port.model(), port.current()]).then(([name, current]) => {
      if (active) { setModel(name); if (!receivedView) setView(current); }
    }).catch(error => { if (active) report(error); });
    void port.onView(next => { receivedView = true; if (active) setView(next); }).then(dispose => {
      if (active) release = dispose; else dispose();
    }).catch(error => { if (active) report(error); });
    return () => { active = false; release?.(); };
  }, [port, report]);
  const terminal = view?.phase === "finished" || view?.phase === "cancelled";
  useEffect(() => {
    if (view) void port.resultReady().catch(report);
  }, [view, port, report]);
  const perform = async (action: () => Promise<GenerationView | null>) => {
    if (busy) return;
    setBusy(true); setError(null);
    try { const next = await action(); if (next !== null) setView(next); } catch (error) { report(error); }
    finally { setBusy(false); }
  };
  const close = () => { if (!busy) void port.close().catch(report); };
  const issues = view?.items.filter(item => terminal ? item.status !== "completed" : item.conflict || item.problems.length > 0 || item.status === "ignored") ?? [];
  return <OwnedWindowShell controls="close" context="Gerar Projetos em lote" width={error !== null ? 520 : terminal && issues.length === 0 ? 400 : 800}>
    <div hidden={error !== null}>
    {!view ? <GenerationConfiguration model={model} port={port} busy={busy} onError={report} onClose={close}
      onSubmit={options => void perform(() => port.prepare(options))} /> : terminal && issues.length === 0 ? <MessageDialog tone="success" title="Geração concluída"
      description={`${view.items.length} ${view.items.length === 1 ? "Projeto gerado" : "Projetos gerados"}.`}
      primaryAction={{ label: "Fechar", disabled: busy, onClick: close }} /> : !terminal && issues.length === 0 ? <MessageDialog tone="success" title="Pronto para gerar"
      description="Pastas verificadas. Confirme para iniciar."
      primaryAction={{ label: "Continuar Geração", disabled: busy || !view.canContinue, onClick: () => void perform(() => port.run()) }}
      secondaryAction={{ label: "Fechar", disabled: busy, onClick: close }} /> : <ProblemsDialog
      title={terminal ? view.phase === "cancelled" ? "Geração cancelada" : "Resultado da geração" : "Problemas na Geração"}
      description={terminal ? `${view.items.filter(item => item.status === "completed").length} de ${view.items.length} Projetos gerados.` : "Resolva ou ignore as pendências para continuar."}
      columns={terminal ? ["Projeto", "Situação", "Motivo"] : ["Projeto", "Problema", "Ações"]}
      rows={issues.map(item => terminal ? [
        <span title={item.destination}>{item.name}</span>,
        item.status === "ignored" ? "Ignorado" : item.status === "failed" ? "Falhou" : "Não gerado",
        item.problems.join(" ") || "—",
      ] : [
        <span title={item.destination}>{item.name}</span>,
        item.problems.length > 0 ? item.problems.join(" ") : item.decision === "replace" ? "Será sobrescrito" : item.decision === "ignore" ? "Ignorado" : "Já existe no destino",
        <div className="generation-row-actions">
          {item.conflict && <ActionButton disabled={busy || !item.canReplace} onClick={() => void perform(() => port.decide(item.id, "replace"))}>Sobrescrever</ActionButton>}
          <ActionButton disabled={busy || item.status === "ignored"} onClick={() => void perform(() => port.decide(item.id, "ignore"))}>Ignorar</ActionButton>
        </div>,
      ])}
      toolbar={!terminal && <div className="generation-global-actions">
        <ActionButton disabled={busy || !view.items.some(item => item.canReplace)} onClick={() => void perform(() => port.decide(null, "replace"))}>Sobrescrever todos</ActionButton>
        <ActionButton disabled={busy} onClick={() => void perform(() => port.decide(null, "ignore"))}>Ignorar todos</ActionButton>
      </div>}
      closeDisabled={busy} onClose={close} actions={!terminal && <>
        <ActionButton disabled={busy} onClick={() => void perform(() => port.recheck())}>Verificar novamente</ActionButton>
        <ActionButton variant="primary" disabled={busy || !view.canContinue} onClick={() => void perform(() => port.run())}>Continuar Geração</ActionButton>
      </>} />}
    </div>
    {error !== null && <MessageDialog tone="error" title="Não foi possível concluir" description={error}
      primaryAction={{ label: "Voltar", onClick: () => setError(null) }} />}
  </OwnedWindowShell>;
}

function GenerationConfiguration({ model, port, busy, onSubmit, onError, onClose }: {
  model: string; port: ProjectGenerationPort; busy: boolean; onSubmit(options: GenerationOptions): void; onError(error: unknown): void; onClose(): void;
}) {
  const [source, setSource] = useState("");
  const [destination, setDestination] = useState("");
  const [count, setCount] = useState<number | null>(null);
  useEffect(() => {
    let current = true; setCount(null);
    const timer = window.setTimeout(() => {
      if (source.trim()) void port.count(source.trim()).then(value => { if (current) setCount(value); }).catch(() => { if (current) setCount(null); });
    }, 300);
    return () => { current = false; window.clearTimeout(timer); };
  }, [source, port, onError]);
  const choose = async (target: "source" | "destination") => {
    try { const chosen = await port.chooseFolder(); if (chosen !== null) (target === "source" ? setSource : setDestination)(chosen); }
    catch (error) { onError(error); }
  };
  return <div className="ui-operation-dialog"><DialogWindowFrame title="Gerar Projetos em lote" layout="form" actions={<>
    <span className="ui-operation-form__summary" aria-live="polite">{count === null ? "" : `${count} ${count === 1 ? "Projeto" : "Projetos"}`}</span>
    <ActionButton disabled={busy} onClick={onClose}>Cancelar</ActionButton>
    <ActionButton variant="primary" disabled={busy || !model || !source.trim() || !destination.trim()} onClick={() => onSubmit({ sourceFolder: source.trim(), destinationFolder: destination.trim() })}>Verificar e gerar</ActionButton>
  </>}>
    <form className="ui-operation-form" onSubmit={event => event.preventDefault()} aria-busy={busy}>
      <div className="generation-model" aria-label="Projeto modelo" title="Inclui as alterações ainda não salvas. O modelo permanece inalterado.">
        <span className="generation-model__icon"><AppIcon icon={PanelsTopLeft} size={18} /></span>
        <div className="generation-model__identity"><span>Projeto modelo</span><strong title={model}>{model || "\u00a0"}</strong></div>
      </div>
      <fieldset className="ui-operation-form__section" disabled={busy}><legend title="Cada pasta com fotos gera um Projeto, incluindo subpastas. As novas fotos entram somente no Painel.">Pasta de origem</legend>
        <div className="ui-operation-form__destination"><TextInput className="ui-field-control" aria-label="Pasta de origem" placeholder="Selecione a pasta com as fotos" value={source} title={source} onChange={event => setSource(event.target.value)} /><ActionButton variant="primary" onClick={() => void choose("source")}>Escolher…</ActionButton></div>
      </fieldset>
      <fieldset className="ui-operation-form__section" disabled={busy}><legend title="A hierarquia da origem será mantida. Escolha um destino fora da origem.">Pasta de destino</legend>
        <div className="ui-operation-form__destination"><TextInput className="ui-field-control" aria-label="Pasta de destino" placeholder="Selecione onde salvar os projetos" value={destination} title={destination} onChange={event => setDestination(event.target.value)} /><ActionButton variant="primary" onClick={() => void choose("destination")}>Escolher…</ActionButton></div>
      </fieldset>
    </form>
  </DialogWindowFrame></div>;
}
