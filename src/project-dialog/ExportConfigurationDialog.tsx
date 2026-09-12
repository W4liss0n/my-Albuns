import { useEffect, useId, useRef, useState } from "react";
import type { ProjectDialogAction, ProjectDialogState } from "../application/projectDialogPort";
import type { ExportFormat } from "../application/normalExport";
import { ActionButton } from "../ui";
import { DialogWindowFrame } from "../ui/DialogWindowFrame";
import { DialogFocusScope } from "../ui/DialogFocusScope";
import "./ExportConfigurationDialog.css";

type State = Extract<ProjectDialogState, { kind: "exportConfiguration" }>;
export function ExportConfigurationDialog({ state, onAction }: { state: State; onAction(action: ProjectDialogAction): void }) {
  const [options, setOptions] = useState(state.options);
  const [scope, setScope] = useState(state.options.scope);
  const initialFocus = useRef<HTMLInputElement>(null);
  const [first, setFirst] = useState(String(state.sheets.find(sheet => sheet.sheetId === state.options.sheetIds[0])?.number ?? 1));
  const [last, setLast] = useState(String(state.sheets.find(sheet => sheet.sheetId === state.options.sheetIds[state.options.sheetIds.length - 1])?.number ?? state.sheets.length));
  const [quality, setQuality] = useState(100);
  const id = useId();
  useEffect(() => { setOptions(state.options); }, [state.options]);
  const start = Number(first), end = Number(last);
  const validRange = Number.isInteger(start) && Number.isInteger(end) && start >= 1 && start <= end && end <= state.sheets.length;
  const selected = scope === "album" ? state.sheets : validRange ? state.sheets.filter(sheet => sheet.number >= start && sheet.number <= end) : [];
  const count = selected.reduce((sum, sheet) => sum + (options.mode === "sheet" ? 1 : sheet.pageCount), 0);
  const request = { ...options, scope, sheetIds: selected.map(sheet => sheet.sheetId), overwrite: false,
    format: options.format.kind === "jpeg" ? { kind: "jpeg" as const, quality } : options.format };
  const setFormat = (kind: ExportFormat["kind"]) => setOptions(current => ({ ...current, format: kind === "jpeg" ? { kind, quality } : { kind } }));
  return <DialogFocusScope focusKey={`export-${state.busy}`} initialFocusRef={initialFocus} onEscape={() => { if (!state.busy) onAction("dismissExport"); }}><DialogWindowFrame layout="problems" title="Exportar" actions={<>
    <span className="export-configuration__summary" aria-live="polite">{options.format.kind === "pdf" ? `1 PDF · ${count} ${count === 1 ? "página" : "páginas"}` : `${count} ${count === 1 ? "arquivo" : "arquivos"}`}</span>
    <ActionButton disabled={state.busy} onClick={() => onAction("dismissExport")}>Cancelar</ActionButton>
    <ActionButton variant="primary" disabled={state.busy || count === 0 || !options.destination.trim()} onClick={() => onAction({ configureExport: request })}>Exportar</ActionButton>
  </>}>
    <form className="export-configuration" onSubmit={event => event.preventDefault()}>
      <fieldset disabled={state.busy}><legend>Escopo</legend><div className="export-configuration__row">
        <label><input ref={initialFocus} type="radio" name={`${id}-scope`} checked={scope === "album"} onChange={() => setScope("album")} />Álbum inteiro</label>
        <label><input type="radio" name={`${id}-scope`} checked={scope === "range"} onChange={() => setScope("range")} />Intervalo de lâminas</label>
      </div>{scope === "range" && <>
        <div className="export-configuration__range"><label>De<input aria-label="Lâmina inicial" type="number" min={1} max={state.sheets.length} value={first} onChange={event => setFirst(event.target.value)} /></label>
          <label>até<input aria-label="Lâmina final" type="number" min={1} max={state.sheets.length} value={last} onChange={event => setLast(event.target.value)} /></label>
        </div>
        {!validRange ? <p role="alert" className="export-configuration__error">Escolha um intervalo entre 1 e {state.sheets.length}, com início menor ou igual ao fim.</p> : null}
        <p className="export-configuration__hint">Arquivos fora do intervalo serão mantidos. Os nomes existentes não indicam se foram exportados por lâmina ou por página.</p>
      </>}</fieldset>
      <fieldset disabled={state.busy}><legend>Modo</legend><div className="export-configuration__row">
        <label><input type="radio" name={`${id}-mode`} checked={options.mode === "sheet"} onChange={() => setOptions(current => ({ ...current, mode: "sheet" }))} />Por lâmina</label>
        <label><input type="radio" name={`${id}-mode`} checked={options.mode === "page"} onChange={() => setOptions(current => ({ ...current, mode: "page" }))} />Por página</label>
      </div></fieldset>
      <fieldset disabled={state.busy}><legend>Formato</legend><div className="export-configuration__row">
        {(["jpeg", "png", "pdf"] as const).map(kind => <label key={kind}><input type="radio" name={`${id}-format`} checked={options.format.kind === kind} onChange={() => setFormat(kind)} />{kind.toUpperCase()}</label>)}
      </div>{options.format.kind === "jpeg" && <label className="export-configuration__quality">Qualidade
        <input aria-label="Qualidade JPEG" type="range" min={1} max={100} step={1} value={quality} onChange={event => setQuality(Number(event.target.value))} onDoubleClick={() => setQuality(100)} /><output>{quality}%</output>
      </label>}</fieldset>
      <fieldset disabled={state.busy}><legend>Destino</legend><div className="export-configuration__destination">
        <input aria-label="Pasta de destino" value={options.destination} onChange={event => setOptions(current => ({ ...current, destination: event.target.value }))} />
        <ActionButton onClick={() => onAction({ chooseExportDestination: { ...request, sheetIds: options.sheetIds } })}>Escolher…</ActionButton>
      </div></fieldset>
      {state.message && <p role="alert" className="export-configuration__error">{state.message}</p>}
    </form>
  </DialogWindowFrame></DialogFocusScope>;
}
