import { photoshopErrorMessage, type PhotoshopSettingsPort, type PhotoshopStatus } from "../application/photoshop";
import { ActionButton, InlineNotice } from "../ui";
import { useSettingsStatus } from "./useSettingsStatus";

export function PhotoshopSettings({ port }: { port: PhotoshopSettingsPort }) {
  const { status, pending, error, update: updateStatus } = useSettingsStatus(port, photoshopErrorMessage);
  const update = (operation: () => Promise<PhotoshopStatus | null>) => updateStatus(operation, (next) => next);

  const selected = status?.installations.find((installation) => installation.id === status.selectedInstallationId);
  return <section aria-label="Photoshop" className="application-settings-panel" aria-busy={pending}>
    <h2>Photoshop</h2>
    <div className="application-settings-integration">
      <label className="application-settings-field">
        <span>Versão utilizada</span>
        <select className="ui-field-control" disabled={pending || !status?.installations.length} value={status?.selectedInstallationId ?? ""}
        onChange={(event) => { const id = event.currentTarget.value; void update(() => port.select(id)); }}>
          {!status?.installations.length && <option value="">{status ? "Nenhuma instalação encontrada" : "Buscando…"}</option>}
          {status?.installations.map((installation) => <option key={installation.id} value={installation.id}>{installation.name} · {installation.version}</option>)}
        </select>
      </label>
      <ActionButton className="application-settings-locate" disabled={pending} onClick={() => void update(() => port.locate())}>Localizar…</ActionButton>
      {selected && <p className="application-settings-path ui-copyable-text" title={selected.path}>{selected.path}</p>}
    </div>
    {error && <InlineNotice role="alert" tone="error">{error}</InlineNotice>}
  </section>;
}
