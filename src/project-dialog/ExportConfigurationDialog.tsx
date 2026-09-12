import { useEffect, useId, useRef, useState } from "react";
import type { ProjectDialogAction, ProjectDialogState } from "../application/projectDialogPort";
import type { ExportFormat } from "../application/normalExport";
import { ActionButton } from "../ui";
import { DialogWindowFrame } from "../ui/DialogWindowFrame";
import { DialogFocusScope } from "../ui/DialogFocusScope";
import { TextInput } from "../ui/TextInput";
import "./ExportConfigurationDialog.css";

type State = Extract<ProjectDialogState, { kind: "exportConfiguration" }>;

export function ExportConfigurationDialog({ state, onAction }: {
  state: State;
  onAction(action: ProjectDialogAction): void;
}) {
  const [options, setOptions] = useState(state.options);
  const [scope, setScope] = useState(state.options.scope);
  const initialFocus = useRef<HTMLInputElement>(null);
  const [interval, setInterval] = useState(() => {
    const first = state.sheets.find(sheet => sheet.sheetId === state.options.sheetIds[0])?.number ?? 1;
    const last = state.sheets.find(sheet => sheet.sheetId === state.options.sheetIds[state.options.sheetIds.length - 1])?.number ?? state.sheets.length;
    return first === last ? String(first) : `${first}-${last}`;
  });
  const [quality, setQuality] = useState(100);
  const id = useId();
  useEffect(() => { setOptions(state.options); }, [state.options]);

  const range = /^\s*(\d+)(?:\s*[-–]\s*(\d+))?\s*$/.exec(interval);
  const start = Number(range?.[1]), end = Number(range?.[2] ?? range?.[1]);
  const validRange = Number.isInteger(start) && Number.isInteger(end) && start >= 1 && start <= end && end <= state.sheets.length;
  const selected = scope === "album" ? state.sheets : validRange
    ? state.sheets.filter(sheet => sheet.number >= start && sheet.number <= end) : [];
  const count = selected.reduce((sum, sheet) => sum + (options.mode === "sheet" ? 1 : sheet.pageCount), 0);
  const request = {
    ...options, scope, sheetIds: selected.map(sheet => sheet.sheetId), overwrite: false,
    format: options.format.kind === "jpeg" ? { kind: "jpeg" as const, quality } : options.format,
  };
  const setFormat = (kind: ExportFormat["kind"]) => setOptions(current => ({
    ...current, format: kind === "jpeg" ? { kind, quality } : { kind },
  }));
  const summary = options.format.kind === "pdf"
    ? `1 PDF · ${count} ${count === 1 ? "página" : "páginas"}`
    : `${count} ${count === 1 ? "arquivo" : "arquivos"}`;

  return (
    <DialogFocusScope className="export-dialog" focusKey={`export-${state.busy}`}
      initialFocusRef={initialFocus} onEscape={() => { if (!state.busy) onAction("dismissExport"); }}>
      <DialogWindowFrame layout="form" title="Exportar" actions={
        <>
          <div className="export-configuration__summary" aria-live="polite">
            <strong>{summary}</strong>
            <span>{options.format.kind.toUpperCase()} · {options.mode === "sheet" ? "Por lâmina" : "Por página"}</span>
          </div>
          <ActionButton disabled={state.busy} onClick={() => onAction("dismissExport")}>Cancelar</ActionButton>
          <ActionButton variant="primary" disabled={state.busy || count === 0 || !options.destination.trim()}
            onClick={() => onAction({ configureExport: request })}>Exportar</ActionButton>
        </>
      }>
        <form className="export-configuration" onSubmit={event => event.preventDefault()}>
          <fieldset className="export-configuration__section" disabled={state.busy}>
            <legend>Modo</legend>
            <div className="export-configuration__choices">
              <label><input ref={options.mode === "sheet" ? initialFocus : undefined} type="radio" name={`${id}-mode`} checked={options.mode === "sheet"}
                onChange={() => setOptions(current => ({ ...current, mode: "sheet" }))} /><span>Por lâmina</span></label>
              <label><input ref={options.mode === "page" ? initialFocus : undefined} type="radio" name={`${id}-mode`} checked={options.mode === "page"}
                onChange={() => setOptions(current => ({ ...current, mode: "page" }))} /><span>Por página</span></label>
            </div>
          </fieldset>

          <fieldset className="export-configuration__section" disabled={state.busy}>
            <legend>Formato</legend>
            <div className="export-configuration__choices export-configuration__formats">
              {(["jpeg", "png", "pdf"] as const).map(kind => <label key={kind}>
                <input type="radio" name={`${id}-format`} checked={options.format.kind === kind} onChange={() => setFormat(kind)} />
                <span>{kind.toUpperCase()}</span>
              </label>)}
            </div>
            {options.format.kind === "jpeg" && <div className="export-configuration__quality ui-range-control">
              <div className="ui-range-control__heading">
                <label htmlFor={`${id}-quality`}>Qualidade JPEG</label><output htmlFor={`${id}-quality`}>{quality}%</output>
              </div>
              <input id={`${id}-quality`} className="ui-range" aria-label="Qualidade JPEG" type="range"
                min={1} max={100} step={1} value={quality} onChange={event => setQuality(Number(event.target.value))}
                onDoubleClick={() => setQuality(100)} title="Dois cliques para restaurar 100%" />
            </div>}
          </fieldset>

          <fieldset className="export-configuration__section export-configuration__section--destination" disabled={state.busy}>
            <legend>Destino</legend>
            <div className="export-configuration__destination">
              <TextInput className="ui-field-control" aria-label="Pasta de destino" value={options.destination}
                title={options.destination} onChange={event => setOptions(current => ({ ...current, destination: event.target.value }))} />
              <ActionButton onClick={() => onAction({ chooseExportDestination: { ...request, sheetIds: options.sheetIds } })}>Escolher…</ActionButton>
            </div>
          </fieldset>
          <fieldset className="export-configuration__interval" disabled={state.busy}>
            <div className="export-configuration__range">
              <label className="export-configuration__scope-toggle">
                <input type="checkbox" checked={scope === "range"}
                  onChange={event => setScope(event.target.checked ? "range" : "album")} />
                Intervalo de lâminas
              </label>
              <TextInput className="ui-field-control" aria-label="Lâminas do intervalo"
                aria-invalid={scope === "range" && !validRange} disabled={scope !== "range"}
                aria-describedby={scope === "range" ? `${id}-range-help` : undefined}
                placeholder="Ex.: 3-8" title="Uma lâmina (3) ou um intervalo (3-8)"
                value={interval} onChange={event => setInterval(event.target.value)} />
            </div>
            {scope === "range" && <div id={`${id}-range-help`} className="export-configuration__range-help">
              {!validRange && <p role="alert" className="export-configuration__error">
                Informe uma lâmina ou um intervalo de 1 a {state.sheets.length}, como 1-{state.sheets.length}.
              </p>}
              <p className="export-configuration__hint">Arquivos fora do intervalo serão mantidos. Os nomes existentes não indicam se foram exportados por lâmina ou por página.</p>
            </div>}
          </fieldset>
          {state.busy && <p role="status" className="export-configuration__status">Preparando exportação…</p>}
          {state.message && <p role="alert" className="export-configuration__status export-configuration__error">{state.message}</p>}
        </form>
      </DialogWindowFrame>
    </DialogFocusScope>
  );
}
