import assert from "node:assert/strict";
import { sameProcessInstance } from "./DevLifecycleProcessInstances.mjs";

export const PROJECT_CLOSE_SCENARIO = "saved-original-close";
const HOST_STAGES = new Set([
  "project_close_command_received", "project_close_session_acquired",
  "project_close_recovery_finishing", "project_close_recovery_finished",
  "clean_project_close_requested", "project_window_destroyed",
]);

export function summarizeCloseStages(records, originalHost, requestedAtUtc) {
  const duringAttempt = records.filter((record) => record.timestamp >= requestedAtUtc);
  const hostEvents = duringAttempt.filter((record) =>
    Number(record.process_id) === originalHost?.processId && HOST_STAGES.has(record.event),
  );
  return {
    hostEvents,
    lastHostStage: hostEvents.at(-1)?.event ?? null,
    // Frontend events have no process id. Preserve them as clues, not Host identity proof.
    frontendEvents: duringAttempt.filter((record) => record.component === "project-close"),
  };
}

export function assertProjectCloseEvidence(evidence) {
  const { originalHost, copyHost, before, after, aliveBeforeCleanup, copyUi, records, requestedAtUtc } = evidence;
  assert.ok(originalHost?.creationTimeUtc && copyHost?.creationTimeUtc, "Both Host instances must be identified");
  assert.notEqual(originalHost.processId, copyHost.processId, "Original and copy require distinct Host processes");
  assert.ok(before.original.projectId && before.copy.projectId, "Both saved identities must be observed");
  assert.notEqual(before.original.projectId, before.copy.projectId, "Save As must adopt an independent identity");
  assert.equal(before.original.dpi, 320, "The original must be independently saved at 320 DPI");
  assert.equal(before.copy.dpi, 420, "The copy must be independently saved at 420 DPI");
  assert.ok(Number.isSafeInteger(before.original.revision) && before.original.revision > 0);
  assert.ok(Number.isSafeInteger(before.copy.revision) && before.copy.revision > 0);
  for (const name of ["original", "copy"]) {
    assert.match(before[name].sha256, /^[a-f0-9]{64}$/u);
    assert.deepEqual(after[name], before[name], `Closing must preserve the saved ${name}`);
  }
  assert.equal(aliveBeforeCleanup.some((item) => sameProcessInstance(item, originalHost)), false,
    "The original Host must exit before gate cleanup");
  assert.equal(aliveBeforeCleanup.some((item) => sameProcessInstance(item, copyHost)), true,
    "The exact copy Host must remain alive before gate cleanup");
  assert.equal(copyUi.dpi, "420");
  assert.equal(copyUi.state.readyState, "complete");
  assert.equal(copyUi.state.dialogCount, 0);
  assert.equal(copyUi.state.workspaceInert, false);
  assert.notEqual(copyUi.state.workspaceBusy, "true");
  assert.deepEqual(copyUi.state.alerts, []);
  assert.ok(copyUi.state.commandMenus.some((menu) => menu.label === "Arquivo"));
  assert.ok(copyUi.state.commandMenus.every((menu) => menu.disabled === false));
  assert.equal(evidence.originalWindowsGone, true);
  assert.equal(evidence.replacementGlobalReady, true);
  assert.equal(evidence.replacementGlobal?.parentProcessId, originalHost.processId,
    "The replacement Global must be spawned by the original Host");
  assert.ok(evidence.replacementGlobal.creationTimeUtc >= requestedAtUtc,
    "The replacement Global must belong to this close attempt");
  const stages = summarizeCloseStages(records, originalHost, requestedAtUtc);
  for (const event of ["project_close_command_received", "clean_project_close_requested"]) {
    assert.ok(stages.hostEvents.some((record) => record.event === event && record.window_label === "project"),
      `The exact original Host must record ${event} during this attempt`);
  }
  return {
    scenario: PROJECT_CLOSE_SCENARIO,
    originalHost, copyHost, requestedAtUtc, replacementGlobal: evidence.replacementGlobal,
    originalHostExited: true, copyHostAlive: true, copyResponsive: true,
    savedFilesUnchanged: true, independentIdentities: true,
    cleanCloseObserved: true, replacementGlobalReady: true,
    before, after, stages,
  };
}
