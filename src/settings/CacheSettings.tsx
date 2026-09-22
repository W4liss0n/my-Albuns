import { useEffect, useId, useRef, useState } from "react";
import type { CacheSettingsPort } from "../application/cacheSettings";
import { ActionButton, InlineNotice } from "../ui";
import { useDismissableSurface } from "../ui/useDismissableSurface";
import { useSettingsStatus } from "./useSettingsStatus";

const readErrorMessage = () => "Não foi possível consultar o uso das prévias temporárias. Tente novamente.";
const clearErrorMessage = () => "Não foi possível concluir a limpeza das prévias temporárias. Tente novamente.";

export function formatCacheBytes(bytes: number) {
  if (bytes < 1024 * 1024) return `${new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 }).format(bytes / 1024)} KB`;
  return `${new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 }).format(bytes / (1024 * 1024))} MB`;
}

export function CacheSettings({ port }: { port: CacheSettingsPort }) {
  const { status, pending, error, update } = useSettingsStatus(port, readErrorMessage, clearErrorMessage);
  const [confirmation, setConfirmation] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const clearButton = useRef<HTMLButtonElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const confirmationAnchor = useRef<HTMLElement>(null);
  const confirmationId = useId();
  useEffect(() => {
    if (confirmation) cancelButton.current?.focus();
  }, [confirmation]);
  const cancel = () => {
    setConfirmation(false);
    clearButton.current?.focus({ preventScroll: true });
  };
  useDismissableSurface({
    enabled: confirmation && !pending,
    includeFocusOutside: true,
    rootRef: confirmationAnchor,
    onDismiss: ({ reason, event }) => {
      if (reason === "escape") {
        event.preventDefault();
        event.stopPropagation();
        cancel();
      } else setConfirmation(false);
    },
  });
  const confirm = async () => {
    if (!confirmation) return;
    setMessage(null);
    await update(async () => {
      const outcome = await port.clearAll();
      return { outcome, status: await port.status() };
    }, ({ outcome, status }) => {
      setConfirmation(false);
      setMessage(outcome.kind === "scheduled" ? "As prévias serão limpas quando você abrir o MyAlbuns novamente." : `${formatCacheBytes(outcome.result.freedBytes)} liberados.`);
      return status;
    });
  };
  const feedback = status?.clearAllScheduled ? "As prévias serão limpas quando você abrir o MyAlbuns novamente." : message;
  return <section aria-label="Prévias temporárias" className="application-settings-panel application-settings-panel--cache" aria-busy={pending}>
    <h2>Prévias temporárias</h2>
    <dl className="application-settings-cache">
      <div className="application-settings-cache-row">
        <dt>Espaço ocupado</dt>
        <dd>{status ? formatCacheBytes(status.occupiedBytes) : "Calculando…"}</dd>
        <dd ref={confirmationAnchor} className="application-settings-cache-trigger">
          <ActionButton ref={clearButton} disabled={pending || !status || status.clearAllScheduled}
            aria-haspopup="dialog" aria-expanded={confirmation} aria-controls={confirmation ? confirmationId : undefined}
            onClick={() => setConfirmation((open) => !open)}>Limpar prévias</ActionButton>
          {confirmation && <div id={confirmationId} role="dialog" aria-label="Confirmar limpeza das prévias temporárias"
            aria-describedby={`${confirmationId}-description`}
            className="ui-anchored-tooltip application-settings-cache-confirmation">
            <p id={`${confirmationId}-description`}>As prévias serão recriadas quando necessário. Os álbuns podem demorar mais para abrir.</p>
            <div className="application-settings-actions">
              <ActionButton ref={cancelButton} disabled={pending} onClick={cancel}>Cancelar</ActionButton>
              <ActionButton disabled={pending} variant="primary" onClick={() => void confirm()}>{pending ? "Limpando…" : "Confirmar"}</ActionButton>
            </div>
          </div>}
        </dd>
      </div>
    </dl>
    {feedback && <p role="status">{feedback}</p>}
    {error && <InlineNotice role="alert" tone="error">{error}</InlineNotice>}
  </section>;
}
