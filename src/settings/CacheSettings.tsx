import { useCallback, useEffect, useRef, useState } from "react";
import type { CacheSettingsPort, CacheSettingsStatus } from "../application/cacheSettings";
import { ActionButton, InlineNotice } from "../ui";

export function formatCacheBytes(bytes: number) {
  if (bytes < 1024 * 1024) return `${new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 }).format(bytes / 1024)} KB`;
  return `${new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 }).format(bytes / (1024 * 1024))} MB`;
}

export function CacheSettings({ port }: { port: CacheSettingsPort }) {
  const [status, setStatus] = useState<CacheSettingsStatus | null>(null);
  const [confirmation, setConfirmation] = useState(false);
  const [pending, setPending] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const sequence = useRef(0);
  const running = useRef(false);
  const clearButton = useRef<HTMLButtonElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const previousConfirmation = useRef(confirmation);
  useEffect(() => {
    if (confirmation) cancelButton.current?.focus();
    else if (previousConfirmation.current) clearButton.current?.focus();
    previousConfirmation.current = confirmation;
  }, [confirmation]);
  const refresh = useCallback(async () => {
    if (running.current) return;
    const request = ++sequence.current;
    try { const next = await port.status(); if (request === sequence.current) { setStatus(next); setError(null); } }
    catch { if (request === sequence.current) setError("Não foi possível consultar o uso do Cache. Tente novamente."); }
  }, [port]);
  useEffect(() => {
    void refresh(); window.addEventListener("focus", refresh);
    return () => { sequence.current += 1; window.removeEventListener("focus", refresh); };
  }, [refresh]);
  const confirm = async () => {
    if (!confirmation || running.current) return;
    running.current = true;
    const request = ++sequence.current;
    setPending(true); setMessage(null); setError(null);
    try {
      const outcome = await port.clearAll();
      const next = await port.status();
      if (request === sequence.current) {
        setStatus(next); setConfirmation(false);
        setMessage(outcome.kind === "scheduled" ? "Limpeza agendada para a próxima inicialização." : `${formatCacheBytes(outcome.result.freedBytes)} liberados.`);
      }
    } catch { if (request === sequence.current) setError("Não foi possível concluir a limpeza do Cache. Tente novamente."); }
    finally { running.current = false; if (request === sequence.current) setPending(false); }
  };
  const feedback = status?.clearAllScheduled ? "Limpeza agendada para a próxima inicialização." : message;
  return <section aria-label="Cache dos álbuns" className="application-settings-panel application-settings-panel--cache" aria-busy={pending}>
    <h2>Cache dos álbuns</h2>
    {!confirmation && <dl className="application-settings-cache">
      <div className="application-settings-cache-row">
        <dt>Espaço ocupado</dt>
        <dd>{status ? formatCacheBytes(status.occupiedBytes) : "Calculando…"}</dd>
        <dd><ActionButton ref={clearButton} disabled={pending || !status || status.clearAllScheduled} onClick={() => setConfirmation(true)}>Limpar cache</ActionButton></dd>
      </div>
    </dl>}
    {confirmation && <InlineNotice title="Limpar cache?">
      <p>Se o cache estiver em uso, a limpeza ficará para a próxima inicialização. Os projetos e as fotos originais serão mantidos.</p>
      <div className="application-settings-actions">
        <ActionButton ref={cancelButton} disabled={pending} onClick={() => setConfirmation(false)}>Cancelar</ActionButton>
        <ActionButton disabled={pending} variant="primary" onClick={() => void confirm()}>{pending ? "Limpando…" : "Confirmar"}</ActionButton>
      </div>
    </InlineNotice>}
    {feedback && <p role="status">{feedback}</p>}
    {error && <InlineNotice role="alert" tone="error">{error}</InlineNotice>}
  </section>;
}
