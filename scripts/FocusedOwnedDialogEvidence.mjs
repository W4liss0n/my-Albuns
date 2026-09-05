function parsedTimestamp(value, label) {
  const timestamp = Date.parse(value ?? "");
  if (!Number.isFinite(timestamp)) {
    throw new Error(`The ${label} timestamp is unavailable`);
  }
  return timestamp;
}

export function confirmExternalCopyActivationLifecycle({
  activationTerminals,
  attemptId,
  correlatedTerminals,
  pendingHost,
}) {
  if (typeof attemptId !== "string" || attemptId.length === 0) {
    throw new Error("The external-copy attempt was not dispatched");
  }
  if (
    !pendingHost ||
    !Number.isSafeInteger(pendingHost.processId) ||
    pendingHost.processId <= 0
  ) {
    throw new Error("The external-copy pending Host was not dispatched");
  }
  if (!Array.isArray(correlatedTerminals) || correlatedTerminals.length !== 1) {
    throw new Error(
      "The external-copy activation must have exactly one correlated terminal",
    );
  }
  const correlatedTerminal = correlatedTerminals[0];
  if (
    correlatedTerminal.event !== "external_copy_activation_terminal" ||
    correlatedTerminal.outcome !== "cancelled"
  ) {
    throw new Error("The external-copy correlated terminal is not cancellation");
  }
  if (correlatedTerminal.attempt_id !== attemptId) {
    throw new Error("The external-copy correlated terminal has another attempt");
  }
  if (correlatedTerminal.host_process_id !== pendingHost.processId) {
    throw new Error("The external-copy correlated terminal has another Host");
  }
  if (
    parsedTimestamp(correlatedTerminal.timestamp, "correlated terminal") <=
    parsedTimestamp(pendingHost.creationTimeUtc, "pending Host")
  ) {
    throw new Error(
      "The external-copy correlated terminal is not strictly after the pending Host",
    );
  }
  if (!Array.isArray(activationTerminals) || activationTerminals.length !== 1) {
    throw new Error(
      "The external-copy activation must have exactly one activation terminal",
    );
  }
  const activationTerminal = activationTerminals[0];
  if (
    activationTerminal.event !== "global_activation_batch_completed" ||
    activationTerminal.project_count !== 1 ||
    activationTerminal.opened_count !== 0 ||
    activationTerminal.focused_count !== 0 ||
    activationTerminal.failed_count !== 1
  ) {
    throw new Error(
      "The external-copy public terminal is not the exact cancelled activation",
    );
  }
  if (
    parsedTimestamp(activationTerminal.timestamp, "activation terminal") <
    parsedTimestamp(correlatedTerminal.timestamp, "correlated terminal")
  ) {
    throw new Error(
      "The external-copy activation terminal occurred before its correlated terminal",
    );
  }
  return {
    activationDispatched: true,
    hostCorrelated: true,
    publicTerminalObserved: true,
  };
}
