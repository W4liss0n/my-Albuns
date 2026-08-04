import { useEffect, useRef, useState } from "react";

interface DocumentDpiControlProps {
  dpi: number;
  onApplyDpi(dpi: number): void | Promise<void>;
}

const DPI_ERROR = "Informe um DPI inteiro entre 1 e 1.200.";

function parseDpiDraft(draft: string) {
  if (!/^\d+$/.test(draft)) return null;
  const dpi = Number(draft);
  return Number.isInteger(dpi) && dpi >= 1 && dpi <= 1_200 ? dpi : null;
}

export function DocumentDpiControl({
  dpi,
  onApplyDpi,
}: DocumentDpiControlProps) {
  const [draft, setDraft] = useState(String(dpi));
  const [applying, setApplying] = useState(false);
  const applyingRef = useRef(false);
  const candidate = parseDpiDraft(draft);
  const invalid = candidate === null;

  useEffect(() => {
    setDraft(String(dpi));
  }, [dpi]);

  async function applyDraft() {
    if (candidate === null || candidate === dpi || applyingRef.current) {
      return;
    }
    applyingRef.current = true;
    setApplying(true);
    try {
      await onApplyDpi(candidate);
    } finally {
      applyingRef.current = false;
      setApplying(false);
    }
  }

  return (
    <form
      className="document-dpi-control"
      onSubmit={(event) => {
        event.preventDefault();
        void applyDraft();
      }}
    >
      <label>
        <span>DPI</span>
        <input
          type="text"
          inputMode="numeric"
          value={draft}
          aria-invalid={invalid}
          onChange={(event) => setDraft(event.currentTarget.value)}
        />
      </label>
      {invalid && <span role="alert">{DPI_ERROR}</span>}
      <button
        type="submit"
        disabled={applying || candidate === null || candidate === dpi}
      >
        Aplicar DPI
      </button>
    </form>
  );
}
