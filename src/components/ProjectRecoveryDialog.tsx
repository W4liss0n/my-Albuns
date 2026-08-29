import { useRef } from "react";

import { ConfirmationDialog, InlineNotice } from "../ui";
import { ModalDialogLayer } from "../ui/ModalDialogLayer";

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
      <ModalDialogLayer
        focusKey={state}
        initialFocusRef={primaryActionRef}
        onEscape={onBack}
        owner="project"
      >
        <ConfirmationDialog
          cancelAction={{ label: "Voltar", onClick: onBack }}
          confirmAction={{
            label: "Descartar recuperação e abrir",
            onClick: onDiscard,
          }}
          confirmButtonRef={primaryActionRef}
          description="A última versão salva será aberta e o trabalho recuperável será removido definitivamente."
          title="Descartar o trabalho recuperável?"
          tone="danger"
        >
          {error ? <InlineNotice tone="error">{error}</InlineNotice> : null}
        </ConfirmationDialog>
      </ModalDialogLayer>
    );
  }

  return (
    <ModalDialogLayer
      focusKey={state}
      initialFocusRef={primaryActionRef}
      onEscape={() => {
        if (!busy) onDefer();
      }}
      owner="project"
    >
      <ConfirmationDialog
        cancelAction={{
          disabled: busy,
          label: "Abrir última versão salva",
          onClick: onRequestDiscard,
        }}
        confirmAction={{
          disabled: busy,
          label: "Reabrir e recuperar",
          onClick: onRecover,
        }}
        confirmButtonRef={primaryActionRef}
        description="O MyAlbuns encontrou trabalho concluído depois da última versão salva deste Projeto."
        leadingAction={{
          disabled: busy,
          label: "Agora não",
          onClick: onDefer,
        }}
        title="Recuperar trabalho não salvo?"
      >
        {error ? <InlineNotice tone="error">{error}</InlineNotice> : null}
        {busy ? <p aria-live="polite">Concluindo…</p> : null}
      </ConfirmationDialog>
    </ModalDialogLayer>
  );
}
