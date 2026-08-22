export function isCausalHandoffObserved({
  globalProcessObserved,
  hostProcessObserved,
  globalExited,
  hostAlive,
  hostReady,
  projectUiReady,
  globalExitedAfterProjectHandoff,
}) {
  return Boolean(
    globalProcessObserved &&
      hostProcessObserved &&
      globalExited &&
      hostAlive &&
      hostReady &&
      projectUiReady &&
      globalExitedAfterProjectHandoff,
  );
}

export function assertCausalHandoffObserved(observed) {
  if (!observed) {
    throw new Error(
      "The causal handoff deadline expired before host_ready, project_ui_ready, and global_exited_after_project_handoff were observed",
    );
  }
}

export function isOwnedHostForestObserved({
  hostProcessId,
  hostForest,
  developmentForest,
}) {
  return Boolean(
    Number.isInteger(hostProcessId) &&
      hostForest.includes(hostProcessId) &&
      hostForest.some((processId) => processId !== hostProcessId) &&
      hostForest.every((processId) => developmentForest.includes(processId)),
  );
}

export function observesTypedCleanupTerminal(output) {
  return observesLogEvent([output], "dev_environment_cleanup_completed");
}

export function observesLogEvent(outputs, event) {
  return outputs.some(
    (output) =>
      output.includes(`"event":"${event}"`) ||
      output.includes(`event="${event}"`),
  );
}
