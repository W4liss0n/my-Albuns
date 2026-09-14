import { useEffect, useRef, useState } from "react";
import type { BatchExportOptions, BatchExportPort } from "../application/batchExport";
import { ActionButton } from "../ui/ActionButton";
import { DialogWindowFrame } from "../ui/DialogWindowFrame";
import { TextInput } from "../ui/TextInput";
import "../ui/ExportForm.css";
import "./batchExport.css";

export function BatchConfiguration({ port, busy, onSubmit, onError, onClose }: {
  port: BatchExportPort; busy: boolean;
  onSubmit(options: BatchExportOptions): void; onError(error: unknown): void; onClose(): void;
}) {
  const [source, setSource] = useState("");
  const [destination, setDestination] = useState("");
  const [alternate, setAlternate] = useState(false);
  const [format, setFormat] = useState<BatchExportOptions["format"]["kind"]>("jpeg");
  const [mode, setMode] = useState<BatchExportOptions["mode"]>("sheet");
  const [count, setCount] = useState<number | null>(null);
  const generation = useRef(0);
  useEffect(() => {
    const request = ++generation.current;
    setCount(null);
    if (!source.trim()) return;
    const timeout = window.setTimeout(() => {
      void port.countProjects(source.trim()).then(next => {
        if (generation.current === request) setCount(next);
      }).catch(error => { if (generation.current === request) onError(error); });
    }, 300);
    return () => { window.clearTimeout(timeout); generation.current++; };
  }, [source, port, onError]);
  const choose = async (target: "source" | "destination") => {
    try {
      const selected = await port.chooseFolder();
      if (selected !== null) (target === "source" ? setSource : setDestination)(selected);
    } catch (error) { onError(error); }
  };
  return <div className="ui-export-dialog batch-configuration">
    <DialogWindowFrame title="Exportação em lote" layout="form" actions={<>
      <span className="ui-export-form__summary" aria-live="polite">
        {count !== null ? `${count} ${count === 1 ? "Projeto encontrado" : "Projetos encontrados"}` : ""}
      </span>
      <ActionButton disabled={busy} onClick={onClose}>Cancelar</ActionButton>
      <ActionButton variant="primary" disabled={busy || count === null || count === 0 || (alternate && !destination.trim())}
        onClick={() => onSubmit({ sourceFolder: source.trim(), destinationFolder: alternate ? destination.trim() : null,
          format: format === "jpeg" ? { kind: format, quality: 100 } : { kind: format }, mode })}>Verificar e exportar</ActionButton>
    </>}>
      <form className="ui-export-form" onSubmit={event => event.preventDefault()} aria-busy={busy}>
        <fieldset className="ui-export-form__section" disabled={busy}>
          <legend>Pasta dos Projetos</legend>
          <div className="ui-export-form__destination">
            <TextInput className="ui-field-control" aria-label="Pasta dos Projetos" value={source} title={source}
              onChange={event => setSource(event.target.value)} />
            <ActionButton onClick={() => void choose("source")}>Escolher…</ActionButton>
          </div>
        </fieldset>
        <div className="batch-configuration__columns">
          <fieldset className="ui-export-form__section" disabled={busy}>
            <legend>Formato</legend>
            <select className="ui-field-control" aria-label="Formato" value={format}
              onChange={event => setFormat(event.target.value as typeof format)}>
              <option value="jpeg">JPEG</option><option value="png">PNG</option><option value="pdf">PDF</option>
            </select>
          </fieldset>
          <fieldset className="ui-export-form__section" disabled={busy}>
            <legend>Modo</legend>
            <select className="ui-field-control" aria-label="Modo" value={mode}
              onChange={event => setMode(event.target.value as typeof mode)}>
              <option value="sheet">Por lâmina</option><option value="page">Por página</option>
            </select>
          </fieldset>
        </div>
        <fieldset className="ui-export-form__section" disabled={busy}>
          <legend>Destino da exportação</legend>
          <div className="batch-configuration__destination-mode">
            <label><input type="radio" name="batch-destination" checked={!alternate} onChange={() => setAlternate(false)} />Padrão de cada Projeto</label>
            <label><input type="radio" name="batch-destination" checked={alternate} onChange={() => setAlternate(true)} />Outra pasta</label>
          </div>
          <div className="ui-export-form__destination">
            <TextInput className="ui-field-control" aria-label="Pasta de destino" disabled={!alternate}
              placeholder={alternate ? "" : "Ao lado de cada Projeto"} value={alternate ? destination : ""}
              title={destination} onChange={event => setDestination(event.target.value)} />
            <ActionButton disabled={!alternate} onClick={() => void choose("destination")}>Escolher…</ActionButton>
          </div>
        </fieldset>
      </form>
    </DialogWindowFrame>
  </div>;
}
