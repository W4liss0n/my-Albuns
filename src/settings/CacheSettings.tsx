import { useCallback, useEffect, useRef, useState } from "react";
import type { CacheSettingsPort, CacheSettingsStatus } from "../application/cacheSettings";
import { ActionButton, InlineNotice } from "../ui";

export function formatCacheBytes(bytes: number) {
  if (bytes < 1024 * 1024) return `${new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 }).format(bytes / 1024)} KB`;
  return `${new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 }).format(bytes / (1024 * 1024))} MB`;
}

export function CacheSettings({ port }: { port: CacheSettingsPort }) {
  const [status, setStatus] = useState<CacheSettingsStatus | null>(null);
  const [confirmation, setConfirmation] = useState<"closed" | "all" | null>(null);
  const [pending, setPending] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const sequence = useRef(0);
  const running = useRef(false);
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
      const outcome = confirmation === "closed" ? { kind: "cleared" as const, result: await port.freeClosedProjects() } : await port.clearAll();
      const next = await port.status();
      if (request === sequence.current) {
        setStatus(next); setConfirmation(null);
        setMessage(outcome.kind === "scheduled" ? "Limpeza agendada para a próxima inicialização segura." : `${formatCacheBytes(outcome.result.freedBytes)} liberados.`);
      }
    } catch { if (request === sequence.current) setError("Não foi possível concluir a limpeza do Cache. Tente novamente."); }
    finally { running.current = false; if (request === sequence.current) setPending(false); }
  };
  return <section aria-label="Cache" className="application-settings-panel" aria-busy={pending}>
    <h2>Cache</h2>
    <p>Gerencie o espaço ocupado pelas prévias dos Projetos.</p>
    <dl className="application-settings-metrics">
      <div><dt>Espaço ocupado</dt><dd>{status ? formatCacheBytes(status.occupiedBytes) : "Calculando…"}</dd></div>
      <div><dt>Liberável de Projetos fechados</dt><dd>{status ? formatCacheBytes(status.releasableBytes) : "Calculando…"}</dd></div>
    </dl>
    <div className="application-settings-actions">
      <ActionButton disabled={pending || !status || status.releasableBytes === 0} onClick={() => setConfirmation("closed")}>Liberar espaço</ActionButton>
      <ActionButton disabled={pending || !status || status.clearAllScheduled} onClick={() => setConfirmation("all")}>Limpar todo o Cache</ActionButton>
      <ActionButton disabled={pending} onClick={() => void refresh()}>Atualizar</ActionButton>
    </div>
    {confirmation && <InlineNotice title={confirmation === "closed" ? "Liberar espaço?" : "Limpar todo o Cache?"}>
      <p>{confirmation === "closed" ? `Até ${formatCacheBytes(status?.releasableBytes ?? 0)} de prévias de Projetos fechados podem ser removidos.` : "Se houver um Projeto ou Processador ativo, a limpeza será agendada para a próxima inicialização segura."}</p>
      <p>Projetos e arquivos originais serão preservados.</p>
      <div className="application-settings-actions">
        <ActionButton disabled={pending} onClick={() => setConfirmation(null)}>Cancelar</ActionButton>
        <ActionButton disabled={pending} variant="primary" onClick={() => void confirm()}>{pending ? "Limpando…" : "Confirmar"}</ActionButton>
      </div>
    </InlineNotice>}
    {status?.clearAllScheduled && <p role="status">Limpeza total agendada para a próxima inicialização segura.</p>}
    {message && <p role="status">{message}</p>}
    {error && <InlineNotice role="alert" tone="error">{error}</InlineNotice>}
  </section>;
}
