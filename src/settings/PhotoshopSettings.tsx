import { useCallback, useEffect, useRef, useState } from "react";
import { photoshopErrorMessage, type PhotoshopSettingsPort, type PhotoshopStatus } from "../application/photoshop";
import { ActionButton, InlineNotice } from "../ui";

export function PhotoshopSettings({ port }: { port: PhotoshopSettingsPort }) {
  const [status, setStatus] = useState<PhotoshopStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const request = useRef(0);
  const mutating = useRef(false);

  const refresh = useCallback(async () => {
    if (mutating.current) return;
    const current = ++request.current;
    try {
      const status = await port.status();
      if (current === request.current) { setStatus(status); setError(null); }
    } catch (error) { if (current === request.current) setError(photoshopErrorMessage(error)); }
  }, [port]);

  useEffect(() => {
    void refresh();
    window.addEventListener("focus", refresh);
    return () => { request.current += 1; window.removeEventListener("focus", refresh); };
  }, [refresh]);

  const update = async (operation: () => Promise<PhotoshopStatus | null>) => {
    if (mutating.current) return;
    mutating.current = true;
    const current = ++request.current;
    setPending(true); setError(null);
    try {
      const next = await operation();
      if (next && current === request.current) setStatus(next);
    } catch (error) { if (current === request.current) setError(photoshopErrorMessage(error)); }
    finally { mutating.current = false; if (current === request.current) setPending(false); }
  };

  const selected = status?.installations.find((installation) => installation.id === status.selectedInstallationId);
  return <section aria-label="Photoshop" className="application-settings-panel" aria-busy={pending}>
    <h2>Photoshop</h2>
    <p>Abra a Foto original no Adobe Photoshop e acompanhe as alterações no Álbum.</p>
    <label className="application-settings-field">
      <span>Instalação usada</span>
      <select disabled={pending || !status?.installations.length} value={status?.selectedInstallationId ?? ""}
        onChange={(event) => { const id = event.currentTarget.value; void update(() => port.select(id)); }}>
        {!status?.installations.length && <option value="">{status ? "Nenhuma instalação detectada" : "Procurando instalações…"}</option>}
        {status?.installations.map((installation) => <option key={installation.id} value={installation.id}>{installation.name} · {installation.version}</option>)}
      </select>
    </label>
    {selected && <p className="application-settings-path ui-copyable-text" title={selected.path}>{selected.path}</p>}
    <div className="application-settings-actions">
      <ActionButton disabled={pending} onClick={() => void update(() => port.locate())}>Localizar Photoshop…</ActionButton>
      <ActionButton disabled={pending} onClick={() => void refresh()}>Atualizar</ActionButton>
    </div>
    <p role="status" className="application-settings-status">{pending ? "Salvando preferência…" : status ? selected ? "Integração disponível" : "Photoshop não encontrado. Localize uma instalação para habilitar a integração." : "Procurando instalações do Photoshop…"}</p>
    {error && <InlineNotice role="alert" tone="error">{error}</InlineNotice>}
  </section>;
}
