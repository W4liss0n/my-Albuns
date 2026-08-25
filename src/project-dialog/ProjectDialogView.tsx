import type {
  ProjectDialogAction,
  ProjectDialogState,
} from "../application/projectDialogPort";
import {
  ConfirmationDialog,
  MessageDialog,
  ProgressDialog,
} from "../ui";

interface ProjectDialogViewProps {
  onAction(action: ProjectDialogAction): void;
  state: ProjectDialogState;
}

export function ProjectDialogView({
  onAction,
  state,
}: ProjectDialogViewProps) {
  switch (state.kind) {
    case "albumInformationConfirmation":
      return (
        <ConfirmationDialog
          cancelAction={{
            disabled: state.busy,
            label: "Cancelar",
            onClick: () => onAction("cancelAlbumInformation"),
          }}
          confirmAction={{
            disabled: state.busy,
            label: state.busy ? "Aplicando…" : "Aplicar",
            onClick: () => onAction("confirmAlbumInformation"),
          }}
          description="As alterações serão aplicadas juntas e poderão ser desfeitas em uma única ação."
          title="Aplicar alterações no Álbum?"
        >
          <dl className="album-information-change-list">
            {state.details.map((detail) => {
              const separator = detail.indexOf(":");
              const label =
                separator >= 0 ? detail.slice(0, separator) : "Alteração";
              const value =
                separator >= 0
                  ? detail.slice(separator + 1).trim()
                  : detail;
              return (
                <div className="album-information-change" key={detail}>
                  <dt className="album-information-change__label">
                    {label}
                  </dt>
                  <dd className="album-information-change__value">
                    {value}
                  </dd>
                </div>
              );
            })}
          </dl>
        </ConfirmationDialog>
      );

    case "projectCloseConfirmation":
      return (
        <ConfirmationDialog
          cancelAction={{
            disabled: state.busy,
            label: "Cancelar",
            onClick: () => onAction("cancelProjectClose"),
          }}
          confirmAction={{
            disabled: state.busy,
            label: state.busy ? "Salvando…" : "Salvar e fechar",
            onClick: () => onAction("saveAndClose"),
          }}
          description="O Projeto tem alterações que ainda não foram salvas."
          leadingAction={{
            disabled: state.busy,
            label: "Descartar e fechar",
            onClick: () => onAction("discardAndClose"),
          }}
          title="Salvar alterações antes de fechar?"
        />
      );

    case "projectCloseFailure":
      return (
        <MessageDialog
          description={state.message}
          secondaryAction={{
            label: "Fechar",
            onClick: () => onAction("dismissProjectCloseFailure"),
          }}
          title="Não foi possível fechar o Projeto"
          tone="error"
        />
      );

    case "projectOperationFailure":
      return (
        <MessageDialog
          description={state.message}
          secondaryAction={{
            label: "Fechar",
            onClick: () => onAction("dismissProjectOperationFailure"),
          }}
          title="A operação não foi concluída"
          tone="error"
        />
      );

    case "exportProgress":
      return (
        <ProgressDialog
          cancelAction={
            state.cancellable
              ? {
                  disabled: state.cancelRequested,
                  label: state.cancelRequested
                    ? "Cancelando…"
                    : "Cancelar Exportação",
                  onClick: () => onAction("cancelExport"),
                }
              : undefined
          }
          progress={state.progress}
          title="Exportando"
        />
      );

    case "exportFailure":
      return (
        <MessageDialog
          description={state.message}
          primaryAction={{
            disabled: state.retryDisabled,
            label: "Tentar novamente",
            onClick: () => onAction("retryExport"),
          }}
          secondaryAction={{
            label: "Fechar",
            onClick: () => onAction("dismissExport"),
          }}
          title={
            state.cancelled
              ? "Exportação cancelada"
              : "Exportação não concluída"
          }
          tone="error"
        />
      );

    case "exportSuccess":
      return (
        <MessageDialog
          description={state.message}
          secondaryAction={{
            label: "Fechar",
            onClick: () => onAction("dismissExport"),
          }}
          title="Exportação concluída"
          tone="success"
        />
      );
  }
}
