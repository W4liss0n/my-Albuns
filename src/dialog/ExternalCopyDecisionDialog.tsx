import { useRef } from "react";

import {
  ConfirmationDialog,
  DialogFocusScope,
  InlineNotice,
} from "../ui";

export function ExternalCopyDecisionDialog({
  error,
  onCancel,
  onSaveCopyAs,
  projectName = null,
  resolving,
}: {
  error: string | null;
  onCancel(): void;
  onSaveCopyAs(): void;
  /** Named only when the opening window lists several Projects. */
  projectName?: string | null;
  resolving: boolean;
}) {
  const primaryActionRef = useRef<HTMLButtonElement>(null);

  return (
    <DialogFocusScope
      className="external-copy-decision-dialog-scope"
      focusKey={resolving ? "resolving" : "available"}
      initialFocusRef={primaryActionRef}
      onEscape={() => {
        if (!resolving) onCancel();
      }}
    >
      <ConfirmationDialog
        cancelAction={{
          disabled: resolving,
          label: "Cancelar",
          onClick: onCancel,
        }}
        confirmAction={{
          disabled: resolving,
          label: "Salvar cópia como…",
          onClick: onSaveCopyAs,
        }}
        confirmButtonRef={primaryActionRef}
        description={projectName
          ? `Para editar o projeto ${projectName}, salve uma cópia em outro local. O original será mantido.`
          : "Para editar este arquivo, salve uma cópia em outro local. O original será mantido."}
        title="Cópia externa somente leitura"
      >
        {error ? <InlineNotice tone="error">{error}</InlineNotice> : null}
      </ConfirmationDialog>
    </DialogFocusScope>
  );
}
