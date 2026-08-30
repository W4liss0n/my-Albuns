import { invoke } from "@tauri-apps/api/core";

import type { ProjectRecoveryDecision } from "../application/projectPorts";

export function resolveOpeningRecovery(
  attemptId: string,
  decision: ProjectRecoveryDecision,
) {
  return invoke<void>("resolve_opening_recovery", { attemptId, decision });
}
