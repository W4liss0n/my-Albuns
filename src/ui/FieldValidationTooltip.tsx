import { useCallback, useEffect, useRef, useState } from "react";

export interface FieldValidationEntry {
  field: string;
  messages?: readonly string[];
}

export interface FieldValidationTooltipModel {
  dismiss(): void;
  id: string;
  message: string;
  openField?: string;
  show(field: string): void;
}

export function useFieldValidationTooltip(
  id: string,
  entries: readonly FieldValidationEntry[],
): FieldValidationTooltipModel {
  const message = [
    ...new Set(entries.flatMap(({ messages }) => messages ?? [])),
  ].join("\n");
  const firstInvalidField = entries.find(({ messages }) => messages?.length)
    ?.field;
  const [openField, setOpenField] = useState<string | null>();
  const hasMessage = Boolean(message);
  const openFieldHasError = entries.some(
    ({ field, messages }) => field === openField && Boolean(messages?.length),
  );

  useEffect(() => {
    if (!hasMessage) {
      if (openField !== undefined) setOpenField(undefined);
      return;
    }
    if (
      openField === undefined ||
      (openField !== null && !openFieldHasError)
    ) {
      setOpenField(firstInvalidField);
    }
  }, [firstInvalidField, hasMessage, openField, openFieldHasError]);

  const dismiss = useCallback(() => {
    setOpenField(null);
  }, []);
  const show = useCallback((field: string) => {
    setOpenField(field);
  }, []);

  return {
    dismiss,
    id,
    message,
    openField: openField ?? undefined,
    show,
  };
}

export function fieldValidationTooltipAttributes(
  field: string,
  error: string | undefined,
  tooltip: FieldValidationTooltipModel,
) {
  const invalid = Boolean(error && tooltip.message);
  return {
    "aria-describedby": invalid ? tooltip.id : undefined,
    "aria-invalid": invalid || undefined,
    onClick: invalid ? () => tooltip.show(field) : undefined,
    onFocus: invalid ? () => tooltip.show(field) : undefined,
  };
}

export function FieldValidationTooltip({
  tooltip,
}: {
  tooltip: FieldValidationTooltipModel;
}) {
  return tooltip.message ? (
    <p className="ui-visually-hidden" id={tooltip.id} role="alert">
      {tooltip.message}
    </p>
  ) : null;
}

export function FieldValidationAutoTooltip({
  field,
  tooltip,
}: {
  field: string;
  tooltip: FieldValidationTooltipModel;
}) {
  const elementRef = useRef<HTMLSpanElement>(null);
  const open = tooltip.openField === field;

  useEffect(() => {
    if (!open) return;
    const fieldElement = elementRef.current?.parentElement;
    const dismissOutside = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        !fieldElement?.contains(event.target)
      ) {
        tooltip.dismiss();
      }
    };
    document.addEventListener("pointerdown", dismissOutside);
    return () => document.removeEventListener("pointerdown", dismissOutside);
  }, [open, tooltip.dismiss]);

  return open ? (
    <span
      className="ui-field-validation-auto-tooltip"
      ref={elementRef}
      role="tooltip"
    >
      {tooltip.message}
    </span>
  ) : null;
}
