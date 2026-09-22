import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { ColorArea, ColorSlider, ColorThumb, Dialog, I18nProvider, Popover, SliderTrack, parseColor, type Color } from "react-aria-components";
import { ActionButton } from "./ActionButton";
import { TextInput } from "./TextInput";
import { FieldValidationAutoTooltip, FieldValidationTooltip, fieldValidationTooltipAttributes, useFieldValidationTooltip } from "./FieldValidationTooltip";
import "./ColorPropertyControl.css";

interface ColorPropertyControlProps {
  rgb: string | null;
  label: string;
  disabled?: boolean;
  mixed?: boolean;
  previewColors?: readonly [string, string];
  description?: string;
  defaultRgb?: string;
  onPreview?(rgb: string): void;
  onCommit(rgb: string): void;
  onCancel?(): void;
}

export function ColorPropertyControl({ rgb, disabled = false, mixed = rgb === null, previewColors, description, label, defaultRgb = "#000000", onPreview, onCommit, onCancel }: ColorPropertyControlProps) {
  const [draft, setDraft] = useState<string | null>(null);
  const [color, setColor] = useState(() => parseColor(rgb ?? defaultRgb).toFormat("hsb"));
  const trigger = useRef<HTMLButtonElement>(null);
  const restoreFocus = useRef(false);
  const actions = useRef({ onCancel });
  actions.current = { onCancel };
  const valid = draft !== null && /^#[\da-f]{6}$/i.test(draft);
  const error = draft !== null && !valid ? "Use uma cor no formato #A1B2C3." : undefined;
  const validationTooltip = useFieldValidationTooltip(useId(), [
    { field: "color", messages: error ? [error] : undefined },
  ]);
  const cancel = () => { setDraft(null); onCancel?.(); };
  useLayoutEffect(() => {
    if (draft === null && restoreFocus.current) {
      restoreFocus.current = false;
      trigger.current?.focus({ preventScroll: true });
    }
  }, [draft]);
  useEffect(() => {
    if (disabled) { setDraft(null); actions.current.onCancel?.(); }
  }, [disabled]);
  useEffect(() => () => actions.current.onCancel?.(), []);
  const update = (value: string) => {
    setDraft(value);
    if (/^#[\da-f]{6}$/i.test(value)) {
      setColor(parseColor(value).toFormat("hsb"));
      onPreview?.(value.toUpperCase());
    }
    else onCancel?.();
  };
  const selectColor = (next: Color) => {
    // Keep hue when saturation/brightness is zero; hex alone loses that choice.
    setColor(next);
    const hex = next.toString("hex").toUpperCase();
    setDraft(hex);
    onPreview?.(hex);
  };
  const apply = () => {
    if (!valid || draft === null) return;
    restoreFocus.current = true;
    onCommit(draft.toUpperCase());
    setDraft(null);
  };
  return <I18nProvider locale="pt-BR"><div className="ui-color-property">
    <button type="button" className="ui-color-property-trigger" aria-label={`Cor ${label}`}
      aria-haspopup="dialog" aria-expanded={draft !== null} data-mixed={mixed} data-empty={!mixed && rgb === null}
      title={description ?? (mixed ? "Valores diferentes" : rgb ?? "Escolher cor")} disabled={disabled} ref={trigger}
      style={{ backgroundColor: rgb ?? undefined, backgroundImage: mixed && previewColors ? `linear-gradient(to right, ${previewColors[0]} 50%, ${previewColors[1]} 50%)` : undefined }}
      onClick={() => {
        if (draft !== null) { cancel(); return; }
        const initial = rgb ?? defaultRgb;
        setColor(parseColor(initial).toFormat("hsb"));
        setDraft(initial);
      }} />
    <Popover triggerRef={trigger} isOpen={draft !== null && !disabled} isNonModal placement="bottom start"
      onOpenChange={(open) => { if (!open) cancel(); }} className="ui-color-property-popover ui-floating-surface">
      <Dialog aria-label={`Escolher cor ${label}`} className="ui-color-property-surface">
      <div className="ui-color-property-dialog" onKeyDownCapture={(event) => { if (event.key === "Escape") restoreFocus.current = true; }}>
      <ColorArea aria-label={`Saturação e brilho ${label}`} className="ui-color-property-area"
        colorSpace="hsb" xChannel="saturation" yChannel="brightness" value={color} onChange={selectColor}>
        <ColorThumb className="ui-color-property-thumb" />
      </ColorArea>
      <ColorSlider aria-label={`Tom ${label}`} className="ui-color-property-hue" colorSpace="hsb" channel="hue" value={color} onChange={selectColor}>
        <SliderTrack className="ui-color-property-track"><ColorThumb className="ui-color-property-thumb" /></SliderTrack>
      </ColorSlider>
      <div className="ui-color-property-inputs">
        <span aria-hidden="true" className="ui-color-property-sample" style={{ backgroundColor: color.toString("hex") }} />
        <TextInput type="text" className="ui-field-control" aria-label={`Código da cor ${label}`} value={draft ?? ""} maxLength={7}
          {...fieldValidationTooltipAttributes("color", error, validationTooltip)}
          onMouseEnter={() => { if (error) validationTooltip.show("color"); }}
          autoFocus aria-invalid={!valid} onChange={(event) => update(event.currentTarget.value)}
          onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); event.stopPropagation(); apply(); } }} />
        <FieldValidationAutoTooltip field="color" tooltip={validationTooltip} />
      </div>
      <FieldValidationTooltip tooltip={validationTooltip} />
      <div className="ui-color-property-actions">
        <ActionButton density="compact" variant="quiet" onClick={() => { restoreFocus.current = true; cancel(); }}>Cancelar</ActionButton>
        <ActionButton density="compact" variant="primary" disabled={!valid} onClick={apply}>Aplicar cor</ActionButton>
      </div>
      </div>
      </Dialog>
    </Popover>
  </div></I18nProvider>;
}
