import { useState } from "react";

import type { ProjectRecoveryDecision } from "../application/projectPorts";
import { ProjectRecoveryDialog } from "./ProjectRecoveryDialog";

export function OpeningRecoveryDialog({
  attemptId,
  openedFromLoadingOwner,
  resolveOpeningRecovery,
}: {
  attemptId: string;
  openedFromLoadingOwner: boolean;
  resolveOpeningRecovery(
    attemptId: string,
    decision: ProjectRecoveryDecision,
  ): Promise<void>;
}) {
  const [state, setState] = useState<
    "available" | "confirmDiscard" | "resolving"
  >("available");
  const [error, setError] = useState<string | null>(null);

  const resolve = async (decision: ProjectRecoveryDecision) => {
    if (!attemptId || state === "resolving") return;
    setError(null);
    setState("resolving");
    try {
      await resolveOpeningRecovery(attemptId, decision);
    } catch (reason: unknown) {
      setError(
        reason instanceof Error
          ? reason.message
          : "Não foi possível concluir a escolha de Recuperação.",
      );
      setState(
        decision === "discardCheckpointAndOpenLastSaved"
          ? "confirmDiscard"
          : "available",
      );
    }
  };

  return (
    <div data-opening-owner-transition={String(openedFromLoadingOwner)}>
      <ProjectRecoveryDialog
        error={
          attemptId
            ? error
            : "A tentativa de abertura não está mais disponível."
        }
        onBack={() => {
          setError(null);
          setState("available");
        }}
        onDefer={() => void resolve("nowNot")}
        onDiscard={() =>
          void resolve("discardCheckpointAndOpenLastSaved")
        }
        onRecover={() => void resolve("reopenAndRecover")}
        onRequestDiscard={() => setState("confirmDiscard")}
        state={state}
      />
    </div>
  );
}
