import { useId, useState } from "react";
import { photoshopErrorMessage, type PhotoshopSettingsPort, type PhotoshopStatus } from "../application/photoshop";
import { projectCommandDescriptor, projectCommandShortcutLabel } from "../application/projectCommandCatalog";
import {
  ActionButton,
  FieldValidationAutoTooltip,
  FieldValidationTooltip,
  fieldValidationTooltipAttributes,
  useFieldValidationTooltip,
} from "../ui";
import { SettingsSection } from "./SettingsSection";
import { useSettingsStatus } from "./useSettingsStatus";

const openInPhotoshop = projectCommandDescriptor("open-in-photoshop").label;
const openInPhotoshopShortcut = projectCommandShortcutLabel("open-in-photoshop");

export function PhotoshopSettings({ port }: { port: PhotoshopSettingsPort }) {
  const { status, pending, error, update: updateStatus } = useSettingsStatus(port, photoshopErrorMessage);
  // Errors open the shared validation tooltip on the control whose action failed.
  const [failed, setFailed] = useState<"select" | "locate">("select");
  const update = (action: "select" | "locate", operation: () => Promise<PhotoshopStatus | null>) => {
    setFailed(action);
    return updateStatus(operation, (next) => next);
  };
  const tooltip = useFieldValidationTooltip(useId(), [{ field: failed, messages: error ? [error] : [] }]);
  const errorFor = (field: typeof failed) => (failed === field ? error ?? undefined : undefined);
  const selectId = useId();

  const selected = status?.installations.find((installation) => installation.id === status.selectedInstallationId);
  return <SettingsSection title="Photoshop" busy={pending} action={
    <span className="application-settings-tooltip-anchor application-settings-tooltip-anchor--end">
      <ActionButton {...fieldValidationTooltipAttributes("locate", errorFor("locate"), tooltip)}
        className="application-settings-locate" density="compact" variant="quiet" disabled={pending}
        onClick={() => void update("locate", () => port.locate())}>Localizar…</ActionButton>
      <FieldValidationAutoTooltip field="locate" tooltip={tooltip} />
    </span>
  }>
    <div className="application-settings-field">
      <label htmlFor={selectId}>Versão utilizada</label>
      <span className="application-settings-tooltip-anchor">
        <select {...fieldValidationTooltipAttributes("select", errorFor("select"), tooltip)}
          id={selectId} className="ui-field-control" disabled={pending || !status?.installations.length} value={status?.selectedInstallationId ?? ""}
          onChange={(event) => { const id = event.currentTarget.value; void update("select", () => port.select(id)); }}>
          {!status?.installations.length && <option value="">{status ? "Nenhuma instalação encontrada" : "Buscando…"}</option>}
          {status?.installations.map((installation) => <option key={installation.id} value={installation.id}>{installation.name} · {installation.version}</option>)}
        </select>
        <FieldValidationAutoTooltip field="select" tooltip={tooltip} />
      </span>
      {selected && <p className="application-settings-path ui-copyable-text" title={selected.path}>{selected.path}</p>}
    </div>
    <p className="application-settings-support">
      Usado em {openInPhotoshop}{openInPhotoshopShortcut ? ` (${openInPhotoshopShortcut})` : ""}.
    </p>
    <FieldValidationTooltip tooltip={tooltip} />
  </SettingsSection>;
}
