import type { StorageFullPresentation, StorageRecoveryAction } from "../application/storageRecovery";
import { ConfirmationDialog } from "./ConfirmationDialog";

export function StorageFullDialog({ state, onAction }: {
  state: StorageFullPresentation; onAction(action: StorageRecoveryAction): void;
}) {
  return <ConfirmationDialog title="Espaço insuficiente" tone="neutral"
    description={state.message}
    leadingAction={state.canClearCache ? { label: "Retomar", disabled: state.busy, onClick: () => onAction("resumeStorage") } : undefined}
    cancelAction={{ label: "Cancelar", disabled: state.busy, onClick: () => onAction("cancelStorage") }}
    confirmAction={{ label: state.canClearCache ? "Limpar prévias temporárias e retomar" : "Retomar", disabled: state.busy,
      onClick: () => onAction(state.canClearCache ? "clearStorageCache" : "resumeStorage") }} />;
}
