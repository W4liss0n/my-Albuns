import { useEffect, useId, useRef, useState } from "react";
import { ActionButton } from "./ActionButton";
import { TextInput } from "./TextInput";
import { FieldValidationAutoTooltip, FieldValidationTooltip, fieldValidationTooltipAttributes, useFieldValidationTooltip } from "./FieldValidationTooltip";
import { useDismissableSurface } from "./useDismissableSurface";
import "./ColorPropertyControl.css";

interface ColorPropertyControlProps {
  rgb: string | null;
  label: string;
  disabled: boolean;
  defaultRgb?: string;
  onPreview?(rgb: string): void;
  onCommit(rgb: string): void;
  onCancel?(): void;
}

export function ColorPropertyControl({ rgb, disabled, label, defaultRgb = "#000000", onPreview, onCommit, onCancel }: ColorPropertyControlProps) {
  const [draft, setDraft] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const actions = useRef({ onCancel });
  actions.current = { onCancel };
  const valid = draft !== null && /^#[\da-f]{6}$/i.test(draft);
  const error = draft !== null && !valid ? "Use uma cor no formato #A1B2C3." : undefined;
  const validationTooltip = useFieldValidationTooltip(useId(), [
    { field: "color", messages: error ? [error] : undefined },
  ]);
  const cancel = () => { setDraft(null); onCancel?.(); };
  useEffect(() => { if (disabled) setDraft(null); }, [disabled]);
  useEffect(() => () => actions.current.onCancel?.(), []);
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
    if (/^#[\da-f]{6}$/i.test(value)) onPreview?.(value.toUpperCase());
    else onCancel?.();
  };
  const apply = () => {
    if (!valid || draft === null) return;
    onCommit(draft.toUpperCase());
    setDraft(null);
    trigger.current?.focus({ preventScroll: true });
  };
  return <div className="ui-color-property" ref={root}>
    <button type="button" className="ui-color-property-trigger" aria-label={`Cor ${label}`}
      aria-haspopup="dialog" aria-expanded={draft !== null} data-mixed={rgb === null}
      title={rgb === null ? "Cores diferentes" : rgb} disabled={disabled} ref={trigger}
      style={{ backgroundColor: rgb ?? "transparent" }} onClick={() => { if (draft === null) setDraft(rgb ?? defaultRgb); else cancel(); }} />
    {draft !== null && <div className="ui-color-property-popover ui-floating-surface" role="dialog" aria-label={`Escolher cor ${label}`}>
      <div className="ui-color-property-inputs">
        <input type="color" aria-label={`Selecionar cor ${label}`} value={valid ? draft : defaultRgb}
          onChange={(event) => update(event.currentTarget.value)} />
        <TextInput type="text" className="ui-field-control" aria-label={`Código da cor ${label}`} value={draft} maxLength={7}
          {...fieldValidationTooltipAttributes("color", error, validationTooltip)}
          onMouseEnter={() => { if (error) validationTooltip.show("color"); }}
          autoFocus aria-invalid={!valid} onChange={(event) => update(event.currentTarget.value)}
          onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); event.stopPropagation(); apply(); } }} />
        <FieldValidationAutoTooltip field="color" tooltip={validationTooltip} />
      </div>
      <FieldValidationTooltip tooltip={validationTooltip} />
      <div className="ui-color-property-actions">
        <ActionButton density="compact" variant="quiet" onClick={() => { cancel(); trigger.current?.focus(); }}>Cancelar</ActionButton>
        <ActionButton density="compact" variant="primary" disabled={!valid} onClick={apply}>Aplicar cor</ActionButton>
      </div>
    </div>}
  </div>;
}
