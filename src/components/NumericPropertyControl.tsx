import { useEffect, useId, useRef, useState } from "react";
import "./NumericPropertyControl.css";
import type { PointerDragThreshold } from "../application/projectPorts";

export interface NumericPropertyControlActions {
  disabled: boolean;
  scopeKey: string;
  doubleClickTimeMs: number | null;
  dragThreshold: PointerDragThreshold | null;
  settlement?: { serial: number; reset: boolean };
  onPreview(value: number): void;
  onCommit(value: number): void;
  onCancel(): void;
}

interface NumericPropertyControlProps extends NumericPropertyControlActions {
  value: number | null;
  label: string;
  numberLabel: string;
  sliderLabel: string;
  unit: string;
  minimum: number;
  maximum: number;
  step: number;
  resetValue: number;
  sliderMaximum?: number;
  sliderScale?: number;
  formatValue(value: number): string;
  parseValue(text: string): number | null;
  valueText(value: number): string;
  help: string;
  invalidHelp: string;
}

const SLIDER_KEYS = new Set(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]);

export function NumericPropertyControl(props: NumericPropertyControlProps) {
  const { value, disabled, doubleClickTimeMs, dragThreshold, minimum, maximum, step, resetValue,
    formatValue, sliderScale = 1 } = props;
  const parseValue = (text: string) => {
    const parsed = props.parseValue(text);
    return parsed !== null && Number.isSafeInteger(parsed) && parsed >= minimum && parsed <= maximum ? parsed : null;
  };
  const displayNumber = (value: number) => Number(formatValue(value).replace(",", "."));
  const helpId = useId();
  const rootRef = useRef<HTMLDivElement>(null);
  const numberRef = useRef<HTMLInputElement>(null);
  const actionsRef = useRef(props);
  actionsRef.current = props;
  const [textDraft, setTextDraft] = useState<string | null>(null);
  const [liveValue, setLiveValue] = useState<number | undefined>(undefined);
  const pending = useRef<number | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const press = useRef<{ id: number; x: number; y: number; dragged: boolean } | null>(null);
  const shownValue = liveValue ?? value;
  const invalid = textDraft !== null && parseValue(textDraft) === null;

  function clearTimer() {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
  }

  function preview(value: number) {
    pending.current = value;
    setLiveValue(value);
    actionsRef.current.onPreview(value);
  }

  function finish() {
    clearTimer();
    press.current = null;
    const next = pending.current;
    pending.current = null;
    setTextDraft(null);
    if (next !== null) actionsRef.current.onCommit(next);
  }

  function cancel() {
    clearTimer();
    press.current = null;
    pending.current = null;
    setTextDraft(null);
    setLiveValue(undefined);
    actionsRef.current.onCancel();
  }

  const finishRef = useRef(finish);
  finishRef.current = finish;
  const cancelRef = useRef(cancel);
  cancelRef.current = cancel;

  useEffect(() => {
    if (pending.current === null) setLiveValue(undefined);
  }, [value]);

  useEffect(() => {
    if (disabled) cancelRef.current();
  }, [disabled]);

  useEffect(() => {
    if (!props.settlement) return;
    clearTimer();
    press.current = null;
    if (pending.current !== null) setTextDraft(null);
    pending.current = null;
    if (props.settlement.reset) setLiveValue(undefined);
  }, [props.settlement]);

  useEffect(() => {
    // Capture outside actions before their handlers can enqueue Save or another edit.
    const outsidePress = (event: PointerEvent) => {
      if (event.target instanceof Node && !rootRef.current?.contains(event.target)) finishRef.current();
    };
    const beforeShortcut = (event: KeyboardEvent) => {
      if (!event.ctrlKey && !event.metaKey) return;
      const key = event.key.toLowerCase();
      // Text inputs retain their native text Undo/Redo; project shortcuts own the slider.
      if ((key === "z" || key === "y") && event.target === numberRef.current) return;
      if (["s", "w", "z", "y"].includes(key)) finishRef.current();
    };
    const loseWindow = () => cancelRef.current();
    window.addEventListener("pointerdown", outsidePress, true);
    window.addEventListener("keydown", beforeShortcut, true);
    window.addEventListener("blur", loseWindow);
    return () => {
      window.removeEventListener("pointerdown", outsidePress, true);
      window.removeEventListener("keydown", beforeShortcut, true);
      window.removeEventListener("blur", loseWindow);
      clearTimer();
      actionsRef.current.onCancel();
    };
  }, []);

  return (
    <div className="numeric-property-control" ref={rootRef}>
      <div className="numeric-property-heading">
        <span>{props.label}</span>
        <div className="numeric-property-number">
          <input
            ref={numberRef}
            className="ui-field-control"
            type="text"
            role="spinbutton"
            inputMode="decimal"
            aria-label={props.numberLabel}
            aria-valuemin={displayNumber(minimum)}
            aria-valuemax={displayNumber(maximum)}
            aria-valuenow={shownValue === null ? undefined : displayNumber(shownValue)}
            aria-valuetext={shownValue === null ? "Múltiplos valores" : undefined}
            aria-invalid={invalid}
            aria-describedby={helpId}
            disabled={disabled}
            value={textDraft ?? (shownValue === null ? "" : formatValue(shownValue))}
            placeholder={value === null ? "—" : undefined}
            onChange={(event) => {
              clearTimer();
              const text = event.currentTarget.value;
              setTextDraft(text);
              const next = parseValue(text);
              if (next === null) {
                pending.current = null;
                setLiveValue(undefined);
                actionsRef.current.onCancel();
              } else preview(next);
            }}
            onBlur={() => { if (invalid) cancel(); else finish(); }}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                cancel();
              } else if (event.key === "Enter") {
                event.preventDefault();
                event.stopPropagation();
                if (invalid) cancel(); else finish();
              } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
                event.preventDefault();
                event.stopPropagation();
                const current = textDraft === null ? shownValue : parseValue(textDraft);
                const next = Math.max(minimum, Math.min(maximum, (current ?? resetValue) + (event.key === "ArrowUp" ? step : -step)));
                setTextDraft(formatValue(next));
                preview(next);
              }
            }}
          />
          <span aria-hidden="true">{props.unit}</span>
        </div>
      </div>
      <input
        className="ui-range numeric-property-slider"
        type="range"
        aria-label={props.sliderLabel}
        aria-valuetext={shownValue === null ? "Múltiplos valores" : props.valueText(shownValue)}
        aria-describedby={helpId}
        data-mixed={shownValue === null}
        min={minimum / sliderScale}
        max={Math.min(maximum, Math.max(props.sliderMaximum ?? maximum, shownValue ?? minimum)) / sliderScale}
        step={step / sliderScale}
        value={(shownValue ?? resetValue) / sliderScale}
        disabled={disabled || doubleClickTimeMs === null || dragThreshold === null}
        onChange={(event) => { setTextDraft(null); preview(Math.round(event.currentTarget.valueAsNumber * sliderScale)); }}
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          clearTimer();
          press.current = { id: event.pointerId, x: event.clientX, y: event.clientY, dragged: false };
          event.currentTarget.setPointerCapture(event.pointerId);
        }}
        onPointerMove={(event) => {
          const start = press.current;
          if (!start || start.id !== event.pointerId || !dragThreshold) return;
          if (Math.abs(event.clientX - start.x) > dragThreshold.x || Math.abs(event.clientY - start.y) > dragThreshold.y) {
            start.dragged = true;
          }
        }}
        onPointerUp={(event) => {
          const start = press.current;
          if (!start || start.id !== event.pointerId) return;
          // Implicit release fires lostpointercapture after pointerup; the gesture is already finished.
          press.current = null;
          if (start.dragged) finish();
          else timer.current = setTimeout(finish, (doubleClickTimeMs ?? 0) + 32);
        }}
        onPointerCancel={() => cancel()}
        onLostPointerCapture={(event) => { if (press.current?.id === event.pointerId) cancel(); }}
        onDoubleClick={(event) => {
          event.preventDefault();
          clearTimer();
          setTextDraft(null);
          preview(resetValue);
          finish();
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault();
            event.stopPropagation();
            cancel();
          } else if (SLIDER_KEYS.has(event.key)) clearTimer();
        }}
        onKeyUp={(event) => { if (SLIDER_KEYS.has(event.key)) finish(); }}
        onBlur={finish}
      />
      <p id={helpId} className="numeric-property-help" data-invalid={invalid}>
        {invalid ? props.invalidHelp : props.help}
      </p>
    </div>
  );
}
