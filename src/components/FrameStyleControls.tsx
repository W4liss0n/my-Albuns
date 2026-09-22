import type { DisplayUnit, FrameSnapshot, FrameStyleChange } from "../domain/project";
import { createPhysicalFieldDraft, displayUnitLabel, editPhysicalFieldDraft, formatMicrometers, formatPhysicalMeasurement } from "../application/physicalMeasurements";
import { ActionButton } from "../ui";
import { ColorPropertyControl } from "../ui/ColorPropertyControl";
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
      value={opacity} label="Opacidade" numberLabel="Opacidade em porcentagem" sliderLabel="Opacidade do quadro"
      unit="%" minimum={0} maximum={100} step={1} resetValue={100}
      formatValue={String} parseValue={(text) => /^\d+$/.test(text.trim()) ? Number(text) : null}
      valueText={(value) => `${value}%`} invalidHelp="Use um número inteiro entre 0 e 100."
    />
    <NumericPropertyControl {...number((widthUm) => ({ kind: "borderWidth", widthUm }))}
      value={width} label="Borda" numberLabel={`Espessura da borda em ${displayUnitLabel(unit)}`} sliderLabel="Espessura da borda"
      labelAccessory={<ColorPropertyControl rgb={rgb} label="da borda" disabled={actions.disabled}
        onPreview={(value) => actions.onPreview({ kind: "borderColor", rgb: value })}
        onCommit={(value) => actions.onCommit({ kind: "borderColor", rgb: value })}
        onCancel={actions.onCancel} />}
      unit={displayUnitLabel(unit)} minimum={0} maximum={Number.MAX_SAFE_INTEGER} step={250} resetValue={0}
      sliderMaximum={Math.max(5_000, ...frames.map((frame) => frame.style.borderWidthUm))}
      formatValue={(value) => formatMicrometers(value, unit)} parseValue={parseWidth}
      valueText={(value) => value === 0 ? "sem borda" : formatPhysicalMeasurement(value, unit)}
      invalidHelp={`Use uma medida positiva ou zero em ${displayUnitLabel(unit)}.`}
    />
    <div className="frame-style-origin">
      <p>{source === "album" ? "Usando o padrão do álbum" : source === "custom"
        ? frames.length === 1 ? "Definido neste quadro" : "Definido nestes quadros"
        : "Parte da seleção usa o design do álbum"}</p>
      {customized && <ActionButton density="compact" variant="quiet" disabled={actions.disabled}
        onClick={() => actions.onCommit({ kind: "restoreAlbum" })}>Usar padrão do álbum</ActionButton>}
    </div>
  </div>;
}
