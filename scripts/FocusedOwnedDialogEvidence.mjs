function parsedTimestamp(value, label) {
  const timestamp = Date.parse(value ?? "");
  if (!Number.isFinite(timestamp)) {
    throw new Error(`The ${label} timestamp is unavailable`);
  }
  return timestamp;
}

export function confirmExternalCopyActivationLifecycle({
  attemptId,
  pendingHost,
  terminal,
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
  if (!terminal || terminal.event !== "global_activation_batch_completed") {
    throw new Error("The external-copy public terminal was not observed");
  }
  if (
    terminal.project_count !== 1 ||
    terminal.opened_count !== 0 ||
    terminal.focused_count !== 0 ||
    terminal.failed_count !== 1
  ) {
    throw new Error("The external-copy public terminal is not the exact cancelled activation");
  }
  if (
    parsedTimestamp(terminal.timestamp, "public terminal") <
    parsedTimestamp(pendingHost.creationTimeUtc, "pending Host")
  ) {
    throw new Error("The external-copy public terminal occurred before the pending Host");
  }
  return {
    activationDispatched: true,
    publicTerminalObserved: true,
  };
}
