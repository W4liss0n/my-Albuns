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
          {state.details.map((detail) => (
            <div className="ui-standard-message__detail" key={detail}>
              {detail}
            </div>
          ))}
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
        >
          {state.busy ? <p aria-live="polite">Concluindo…</p> : null}
        </ConfirmationDialog>
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
  }
}
