import { useEffect, useRef, useState } from "react";
import type { DisplayUnit, FrameSnapshot, FrameStyleChange } from "../domain/project";
import { createPhysicalFieldDraft, displayUnitLabel, editPhysicalFieldDraft, formatMicrometers, formatPhysicalMeasurement } from "../application/physicalMeasurements";
import { ActionButton } from "../ui";
import { useDismissableSurface } from "../ui/useDismissableSurface";
import { NumericPropertyControl, type NumericPropertyControlActions } from "./NumericPropertyControl";
import "./FrameStyleControls.css";

export interface FrameStyleControlActions extends Omit<NumericPropertyControlActions, "onPreview" | "onCommit"> {
  onPreview(change: FrameStyleChange): void;
  onCommit(change?: FrameStyleChange): void;
}

interface FrameStyleControlsProps extends FrameStyleControlActions {
  frames: readonly FrameSnapshot[];
  unit: DisplayUnit;
}

function sharedValue<T>(frames: readonly FrameSnapshot[], read: (frame: FrameSnapshot) => T): T | null {
  const first = frames[0];
  return first && frames.every((frame) => read(frame) === read(first)) ? read(first) : null;
}

export function FrameStyleControls({ frames, unit, ...actions }: FrameStyleControlsProps) {
  if (frames.length === 0) return null;
  const opacity = sharedValue(frames, (frame) => frame.style.opacityPercent);
  const width = sharedValue(frames, (frame) => frame.style.borderWidthUm);
  const rgb = sharedValue(frames, (frame) => frame.style.borderRgb);
  const source = sharedValue(frames, (frame) => frame.style.source);
  const customized = frames.some((frame) => frame.style.source === "custom");
  const number = (change: (value: number) => FrameStyleChange) => ({ ...actions,
    onPreview: (value: number) => actions.onPreview(change(value)),
    onCommit: (value: number) => actions.onCommit(change(value)),
  });
  const parseWidth = (text: string) => {
    const draft = editPhysicalFieldDraft(createPhysicalFieldDraft(width ?? 0, unit), text, unit);
    return draft.hasExactValue ? draft.valueUm : null;
  };
  return <div className="frame-style-controls">
    <NumericPropertyControl {...number((opacityPercent) => ({ kind: "opacity", opacityPercent }))}
      value={opacity} label="Opacidade" numberLabel="Opacidade em porcentagem" sliderLabel="Opacidade do Frame"
      unit="%" minimum={0} maximum={100} step={1} resetValue={100}
      formatValue={String} parseValue={(text) => /^\d+$/.test(text.trim()) ? Number(text) : null}
      valueText={(value) => `${value}%`} help="Dois cliques para restaurar 100%" invalidHelp="Use um número inteiro entre 0 e 100."
    />
    <div className="frame-style-border-row">
      <FrameBorderColor rgb={rgb} {...actions} />
      <NumericPropertyControl {...number((widthUm) => ({ kind: "borderWidth", widthUm }))}
        value={width} label="Borda" numberLabel={`Espessura da Borda em ${displayUnitLabel(unit)}`} sliderLabel="Espessura da Borda"
        unit={displayUnitLabel(unit)} minimum={0} maximum={Number.MAX_SAFE_INTEGER} step={250} resetValue={0}
        sliderMaximum={Math.max(5_000, ...frames.map((frame) => frame.style.borderWidthUm))}
        formatValue={(value) => formatMicrometers(value, unit)} parseValue={parseWidth}
        valueText={(value) => value === 0 ? "sem borda" : formatPhysicalMeasurement(value, unit)}
        help={width === 0 ? "sem borda · dois cliques para remover" : "Dois cliques para remover a borda"}
        invalidHelp={`Use uma medida positiva ou zero em ${displayUnitLabel(unit)}.`}
      />
    </div>
    <div className="frame-style-origin">
      <p>{source === "album" ? "Usando o design do álbum" : source === "custom"
        ? frames.length === 1 ? "Definido neste Frame" : "Definido nestes Frames"
        : "Parte da seleção usa o design do álbum"}</p>
      {customized && <ActionButton density="compact" variant="quiet" disabled={actions.disabled}
        onClick={() => actions.onCommit({ kind: "restoreAlbum" })}>Voltar ao design do álbum</ActionButton>}
    </div>
  </div>;
}

function FrameBorderColor({ rgb, disabled, onPreview, onCommit, onCancel }: FrameStyleControlActions & { rgb: string | null }) {
  const [draft, setDraft] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const actions = useRef({ onCancel });
  actions.current = { onCancel };
  const valid = draft !== null && /^#[\da-f]{6}$/i.test(draft);
  const cancel = () => { setDraft(null); onCancel(); };
  useEffect(() => { if (disabled) setDraft(null); }, [disabled]);
  useEffect(() => () => actions.current.onCancel(), []);
  useDismissableSurface({ enabled: draft !== null, rootRef: root, includeFocusOutside: true,
    onDismiss: ({ reason, event }) => {
      cancel();
      if (reason === "escape") {
        event.preventDefault(); event.stopPropagation(); trigger.current?.focus({ preventScroll: true });
      }
    },
  });
  const update = (value: string) => {
    setDraft(value);
    if (/^#[\da-f]{6}$/i.test(value)) onPreview({ kind: "borderColor", rgb: value.toUpperCase() });
    else onCancel();
  };
  const apply = () => {
    if (!valid || draft === null) return;
    onCommit({ kind: "borderColor", rgb: draft.toUpperCase() });
    setDraft(null);
    trigger.current?.focus({ preventScroll: true });
  };
  return <div className="frame-style-color" ref={root}>
    <button type="button" className="frame-style-color-trigger" aria-label="Cor da Borda"
      aria-haspopup="dialog" aria-expanded={draft !== null} data-mixed={rgb === null}
      title={rgb === null ? "Cores diferentes" : rgb} disabled={disabled} ref={trigger}
      style={{ backgroundColor: rgb ?? "transparent" }} onClick={() => { if (draft === null) setDraft(rgb ?? "#000000"); else cancel(); }} />
    {draft !== null && <div className="frame-style-color-popover ui-floating-surface" role="dialog" aria-label="Escolher cor da Borda">
      <div className="frame-style-color-inputs">
        <input type="color" aria-label="Selecionar cor da Borda" value={valid ? draft : "#000000"}
          onChange={(event) => update(event.currentTarget.value)} />
        <input type="text" className="ui-field-control" aria-label="Cor hexadecimal da Borda" value={draft} maxLength={7}
          autoFocus aria-invalid={!valid} onChange={(event) => update(event.currentTarget.value)}
          onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); event.stopPropagation(); apply(); } }} />
      </div>
      <div className="frame-style-color-actions">
        <ActionButton density="compact" variant="quiet" onClick={() => { cancel(); trigger.current?.focus(); }}>Cancelar</ActionButton>
        <ActionButton density="compact" variant="primary" disabled={!valid} onClick={apply}>Aplicar cor</ActionButton>
      </div>
    </div>}
  </div>;
}
