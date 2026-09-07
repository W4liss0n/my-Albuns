import { invoke } from "@tauri-apps/api/core";

import type { ProjectRecoveryDecision } from "../application/projectPorts";
import type { OpeningExternalCopyDecision } from "../global/application/globalProjectPort";
import type { OpeningExternalCopyDecision as IpcOpeningExternalCopyDecision } from "./generated/OpeningExternalCopyDecision";
import type { ProjectRecoveryDecision as IpcProjectRecoveryDecision } from "./generated/ProjectRecoveryDecision";

export function resolveOpeningExternalCopy(
  attemptId: string,
  decision: OpeningExternalCopyDecision,
) {
  return invoke<void>("resolve_opening_external_copy", {
    attemptId,
    decision: decision satisfies IpcOpeningExternalCopyDecision,
  });
}

export function resolveOpeningRecovery(
  attemptId: string,
  decision: ProjectRecoveryDecision,
) {
  return invoke<void>("resolve_opening_recovery", {
    attemptId,
    decision: decision satisfies IpcProjectRecoveryDecision,
  });
}
