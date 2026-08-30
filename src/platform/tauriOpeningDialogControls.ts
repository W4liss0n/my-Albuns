import { invoke } from "@tauri-apps/api/core";

import type { ProjectRecoveryDecision } from "../application/projectPorts";
import type { ProjectRecoveryDecision as IpcProjectRecoveryDecision } from "./generated/ProjectRecoveryDecision";

export function resolveOpeningRecovery(
  attemptId: string,
  decision: ProjectRecoveryDecision,
) {
  return invoke<void>("resolve_opening_recovery", {
    attemptId,
    decision: decision satisfies IpcProjectRecoveryDecision,
  });
}
