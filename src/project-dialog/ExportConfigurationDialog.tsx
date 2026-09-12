import { useEffect, useId, useRef, useState } from "react";
import { ChevronDown } from "lucide-react";
import type { ProjectDialogAction, ProjectDialogState } from "../application/projectDialogPort";
import type { ExportFormat } from "../application/normalExport";
import { ActionButton, AppIcon, FieldValidationAutoTooltip, FieldValidationTooltip, fieldValidationTooltipAttributes, useFieldValidationTooltip } from "../ui";
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
    return state.options.scope === "album" ? "" : first === last ? String(first) : `${first}-${last}`;
  });
  const [quality, setQuality] = useState(100);
  const id = useId();
  useEffect(() => { setOptions(state.options); }, [state.options]);

  const range = /^\s*(\d+)(?:\s*[-–]\s*(\d+))?\s*$/.exec(interval);
  const start = Number(range?.[1]), end = Number(range?.[2] ?? range?.[1]);
  const validRange = Number.isInteger(start) && Number.isInteger(end) && start >= 1 && start <= end && end <= state.sheets.length;
  const rangeHelp = `Informe uma lâmina ou um intervalo de 1 a ${state.sheets.length}, como 1-${state.sheets.length}.`;
  const rangeError = scope === "range" && interval.trim() !== "" && !validRange ? rangeHelp : undefined;
  const rangeTooltip = useFieldValidationTooltip(`${id}-range-help`, [
    { field: "interval", messages: rangeError ? [rangeError] : undefined },
  ]);
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
            <span>{summary}</span>{options.format.kind !== "pdf" && <> · {options.format.kind.toUpperCase()}</>}
          </div>
          <ActionButton disabled={state.busy} onClick={() => onAction("dismissExport")}>Cancelar</ActionButton>
          <ActionButton variant="primary" disabled={state.busy || count === 0 || !options.destination.trim()}
            onClick={() => onAction({ configureExport: request })}>Exportar</ActionButton>
        </>
      }>
        <form className="export-configuration" onSubmit={event => event.preventDefault()}>
          <fieldset className="export-configuration__section export-configuration__section--destination" disabled={state.busy}>
            <legend>Destino da exportação</legend>
            <div className="export-configuration__destination">
              <TextInput ref={initialFocus} className="ui-field-control" aria-label="Pasta de destino" value={options.destination}
                title={options.destination} onChange={event => setOptions(current => ({ ...current, destination: event.target.value }))} />
              <ActionButton variant="primary" onClick={() => onAction({ chooseExportDestination: { ...request, sheetIds: options.sheetIds } })}>Escolher…</ActionButton>
            </div>
          </fieldset>

          <fieldset className="export-configuration__section" disabled={state.busy}>
            <legend>Formato de exportação</legend>
            <div className="export-configuration__format-row">
              <div className="export-configuration__format-select">
                <select className="ui-field-control" aria-label="Formato de exportação" value={options.format.kind}
                  onChange={event => setFormat(event.target.value as ExportFormat["kind"])}>
                  <option value="jpeg">JPEG</option><option value="png">PNG</option><option value="pdf">PDF</option>
                </select>
                <AppIcon icon={ChevronDown} size={16} />
              </div>
              {options.format.kind === "jpeg" && <div className="export-configuration__quality">
                <label htmlFor={`${id}-quality`}>Qualidade:</label>
                <input id={`${id}-quality`} className="ui-range" aria-label="Qualidade JPEG" type="range"
                  min={1} max={100} step={1} value={quality} onChange={event => setQuality(Number(event.target.value))}
                  onDoubleClick={() => setQuality(100)} title="Dois cliques para restaurar 100%" />
                <output htmlFor={`${id}-quality`}>{quality}%</output>
              </div>}
            </div>
          </fieldset>

          <fieldset className="export-configuration__section" disabled={state.busy}>
            <legend>Seleção de lâminas</legend>
            <div className="export-configuration__selection">
              <label className="export-configuration__choice">
                <input type="radio" name={`${id}-scope`} checked={scope === "album"} onChange={() => setScope("album")} />
                Todas as lâminas
              </label>
              <div className="export-configuration__selection-row">
                <div className="export-configuration__range">
                  <label className="export-configuration__choice">
                    <input type="radio" name={`${id}-scope`} checked={scope === "range"} onChange={() => setScope("range")} />
                    Intervalo personalizado
                  </label>
                  <span className="export-configuration__range-field">
                    <TextInput className="ui-field-control" aria-label="Lâminas do intervalo"
                      {...fieldValidationTooltipAttributes("interval", rangeError, rangeTooltip)}
                      disabled={scope !== "range"}
                      placeholder="Ex.: 3-8" title={rangeError ? undefined : rangeHelp}
                      onMouseEnter={() => { if (rangeError) rangeTooltip.show("interval"); }}
                      value={interval} onChange={event => setInterval(event.target.value)} />
                    <FieldValidationAutoTooltip field="interval" tooltip={rangeTooltip} />
                  </span>
                </div>
                <label className="export-configuration__choice export-configuration__page-mode">
                  <input type="checkbox" checked={options.mode === "page"}
                    onChange={event => setOptions(current => ({ ...current, mode: event.target.checked ? "page" : "sheet" }))} />
                  Exportar como páginas simples
                </label>
              </div>
            </div>
            <FieldValidationTooltip tooltip={rangeTooltip} />
          </fieldset>
          {state.busy && <p role="status" className="export-configuration__status">Preparando exportação…</p>}
          {state.message && <p role="alert" className="export-configuration__status export-configuration__error">{state.message}</p>}
        </form>
      </DialogWindowFrame>
    </DialogFocusScope>
  );
}
