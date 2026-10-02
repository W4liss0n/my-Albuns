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
  resolving,
}: {
  error: string | null;
  onCancel(): void;
  onSaveCopyAs(): void;
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
        description="Para editar este arquivo, salve uma cópia em outro local. O original será mantido."
        title="Cópia externa somente leitura"
      >
        {error ? <InlineNotice tone="error">{error}</InlineNotice> : null}
      </ConfirmationDialog>
    </DialogFocusScope>
  );
}
