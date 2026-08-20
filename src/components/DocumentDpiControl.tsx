import { useEffect, useRef, useState } from "react";

interface DocumentDpiControlProps {
  dpi: number;
  formId: string;
  onApplyDpi(dpi: number): void | Promise<void>;
  onDirtyChange(dirty: boolean): void;
}

const DPI_ERROR = "Informe um DPI inteiro entre 1 e 1.200.";

function parseDpiDraft(draft: string) {
  if (!/^\d+$/.test(draft)) return null;
  const dpi = Number(draft);
  return Number.isInteger(dpi) && dpi >= 1 && dpi <= 1_200 ? dpi : null;
}

export function DocumentDpiControl({
  dpi,
  formId,
  onApplyDpi,
  onDirtyChange,
}: DocumentDpiControlProps) {
  const [draft, setDraft] = useState(String(dpi));
  const applyingRef = useRef(false);
  const candidate = parseDpiDraft(draft);
  const invalid = candidate === null;

  useEffect(() => {
    setDraft(String(dpi));
  }, [dpi]);

  useEffect(() => {
    onDirtyChange(candidate !== null && candidate !== dpi);
  }, [candidate, dpi, onDirtyChange]);

  async function applyDraft() {
    if (candidate === null || candidate === dpi || applyingRef.current) {
      return;
    }
    applyingRef.current = true;
    try {
      await onApplyDpi(candidate);
    } finally {
      applyingRef.current = false;
    }
  }

  return (
    <form
      id={formId}
      className="document-dpi-control"
      onSubmit={(event) => {
        event.preventDefault();
        void applyDraft();
      }}
    >
      <label>
        <span>DPI</span>
        <input
          className="ui-field-control"
          type="text"
          inputMode="numeric"
          value={draft}
          aria-invalid={invalid}
          onChange={(event) => setDraft(event.currentTarget.value)}
        />
      </label>
      {invalid && <span role="alert">{DPI_ERROR}</span>}
    </form>
  );
}
