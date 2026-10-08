import { useState } from "react";

import type { OpeningExternalCopyDecision } from "../global/application/globalProjectPort";
import { ExternalCopyDecisionDialog } from "./ExternalCopyDecisionDialog";

export function OpeningExternalCopyDialog({
  attemptId,
  openedFromLoadingOwner,
  projectName = null,
  resolveOpeningExternalCopy,
}: {
  attemptId: string;
  openedFromLoadingOwner: boolean;
  projectName?: string | null;
  resolveOpeningExternalCopy(
    attemptId: string,
    decision: OpeningExternalCopyDecision,
  ): Promise<void>;
}) {
  const [resolving, setResolving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const resolve = async (decision: OpeningExternalCopyDecision) => {
    if (!attemptId || resolving) return;
    setError(null);
    setResolving(true);
    try {
      await resolveOpeningExternalCopy(attemptId, decision);
    } catch (reason: unknown) {
      setError(
        reason instanceof Error
          ? reason.message
          : "Não foi possível concluir a decisão sobre a Cópia externa.",
      );
      setResolving(false);
    }
  };

  return (
    <div data-opening-owner-transition={String(openedFromLoadingOwner)}>
      <ExternalCopyDecisionDialog
        projectName={projectName}
        error={
          attemptId
            ? error
            : "A tentativa de abertura não está mais disponível."
        }
        onCancel={() => void resolve("cancel")}
        onSaveCopyAs={() => void resolve("saveCopyAs")}
        resolving={resolving}
      />
    </div>
  );
}
