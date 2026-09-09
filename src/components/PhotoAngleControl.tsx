import { useEffect, useId, useRef, useState } from "react";
import type { PointerDragThreshold } from "../application/projectPorts";

export interface PhotoAngleControlActions {
  disabled: boolean;
  scopeKey: string;
  doubleClickTimeMs: number | null;
  dragThreshold: PointerDragThreshold | null;
  settlement?: { serial: number; reset: boolean };
  onPreview(angleTenths: number): void;
  onCommit(angleTenths: number): void;
  onCancel(): void;
}

interface PhotoAngleControlProps extends PhotoAngleControlActions {
  value: number | null;
}

const SLIDER_KEYS = new Set(["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]);

function parseAngle(text: string): number | null {
  if (!/^[+-]?\d+(?:[.,]\d)?$/.test(text.trim())) return null;
  const degrees = Number(text.trim().replace(",", "."));
  return Number.isFinite(degrees) && Math.abs(degrees) <= 45 ? Math.round(degrees * 10) : null;
}

function formatAngle(tenths: number) {
  return String(tenths / 10).replace(".", ",");
}

export function PhotoAngleControl(props: PhotoAngleControlProps) {
  const { value, disabled, doubleClickTimeMs, dragThreshold } = props;
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
  const invalid = textDraft !== null && parseAngle(textDraft) === null;

  function clearTimer() {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
  }

  function preview(tenths: number) {
    pending.current = tenths;
    setLiveValue(tenths);
    actionsRef.current.onPreview(tenths);
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
    <div className="photo-angle-control" ref={rootRef}>
      <div className="photo-angle-heading">
        <span>Ângulo</span>
        <div className="photo-angle-number">
          <input
            ref={numberRef}
            className="ui-field-control"
            type="text"
            role="spinbutton"
            inputMode="decimal"
            aria-label="Ângulo em graus"
            aria-valuemin={-45}
            aria-valuemax={45}
            aria-valuenow={shownValue === null ? undefined : shownValue / 10}
            aria-valuetext={shownValue === null ? "Múltiplos valores" : undefined}
            aria-invalid={invalid}
            aria-describedby={helpId}
            disabled={disabled}
            value={textDraft ?? (shownValue === null ? "" : formatAngle(shownValue))}
            placeholder={value === null ? "—" : undefined}
            onChange={(event) => {
              clearTimer();
              const text = event.currentTarget.value;
              setTextDraft(text);
              const next = parseAngle(text);
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
                const current = textDraft === null ? shownValue : parseAngle(textDraft);
                const next = Math.max(-450, Math.min(450, (current ?? 0) + (event.key === "ArrowUp" ? 1 : -1)));
                setTextDraft(formatAngle(next));
                preview(next);
              }
            }}
          />
          <span aria-hidden="true">°</span>
        </div>
      </div>
      <input
        className="ui-range photo-angle-slider"
        type="range"
        aria-label="Ângulo da Foto"
        aria-valuetext={shownValue === null ? "Múltiplos valores" : `${formatAngle(shownValue)} graus`}
        aria-describedby={helpId}
        data-mixed={shownValue === null}
        min={-45}
        max={45}
        step={0.1}
        value={(shownValue ?? 0) / 10}
        disabled={disabled || doubleClickTimeMs === null || dragThreshold === null}
        onChange={(event) => { setTextDraft(null); preview(Math.round(event.currentTarget.valueAsNumber * 10)); }}
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
          preview(0);
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
      <p id={helpId} className="photo-angle-help" data-invalid={invalid}>
        {invalid ? "Use −45° a 45°, com uma casa decimal." : "−45° a 45° · dois cliques para zerar"}
      </p>
    </div>
  );
}
