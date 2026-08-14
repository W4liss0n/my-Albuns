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

export function observesTypedCleanupTerminal(output) {
  return output.includes('"event":"dev_environment_cleanup_completed"');
}
