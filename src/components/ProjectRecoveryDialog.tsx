import { useRef } from "react";

import { ConfirmationDialog, DialogFocusScope, InlineNotice } from "../ui";

import "./ProjectRecoveryDialog.css";

type ProjectRecoveryDialogState =
  | "available"
  | "confirmDiscard"
  | "resolving";

export function ProjectRecoveryDialog({
  error,
  onBack,
  onDefer,
  onDiscard,
  onRecover,
  onRequestDiscard,
  state,
}: {
  error: string | null;
  onBack(): void;
  onDefer(): void;
  onDiscard(): void;
  onRecover(): void;
  onRequestDiscard(): void;
  state: ProjectRecoveryDialogState;
}) {
  const primaryActionRef = useRef<HTMLButtonElement>(null);
  const busy = state === "resolving";

  if (state === "confirmDiscard") {
    return (
      <DialogFocusScope
        className="project-recovery-dialog-scope"
        focusKey={state}
        initialFocusRef={primaryActionRef}
        onEscape={onBack}
      >
        <ConfirmationDialog
          cancelAction={{ label: "Voltar", onClick: onBack }}
          confirmAction={{
            label: "Descartar alterações e abrir",
            onClick: onDiscard,
          }}
          confirmButtonRef={primaryActionRef}
          description="As alterações não salvas serão descartadas definitivamente. O projeto abrirá na última versão salva."
          title="Descartar o trabalho recuperável?"
          tone="danger"
        >
          {error ? <InlineNotice tone="error">{error}</InlineNotice> : null}
        </ConfirmationDialog>
      </DialogFocusScope>
    );
  }

  return (
    <DialogFocusScope
      className="project-recovery-dialog-scope"
      focusKey={state}
      initialFocusRef={primaryActionRef}
      onEscape={() => {
        if (!busy) onDefer();
      }}
    >
      <ConfirmationDialog
        cancelAction={{
          disabled: busy,
          label: "Abrir última versão salva",
          onClick: onRequestDiscard,
        }}
        confirmAction={{
          disabled: busy,
          label: "Recuperar e abrir",
          onClick: onRecover,
        }}
        confirmButtonRef={primaryActionRef}
        description="Há alterações não salvas deste projeto. Deseja recuperá-las?"
        leadingAction={{
          disabled: busy,
          label: "Agora não",
          onClick: onDefer,
        }}
        title="Recuperar trabalho não salvo?"
      >
        {error ? <InlineNotice tone="error">{error}</InlineNotice> : null}
      </ConfirmationDialog>
    </DialogFocusScope>
  );
}
