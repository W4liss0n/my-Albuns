import assert from "node:assert/strict";
import test from "node:test";

import {
  assertCausalHandoffObserved,
  isCausalHandoffObserved,
  observesTypedCleanupTerminal,
} from "./DevLifecycleGateObservations.mjs";

test("an incomplete causal handoff expires fail closed", () => {
  const observed = isCausalHandoffObserved({
    globalProcessObserved: true,
    hostProcessObserved: true,
    globalExited: true,
    hostAlive: true,
    hostReady: true,
    projectUiReady: true,
    globalExitedAfterProjectHandoff: false,
  });

  assert.equal(observed, false);
  assert.throws(
    () => assertCausalHandoffObserved(observed),
    /causal handoff deadline expired/i,
  );
});

test("bootstrap failure evidence requires the typed cleanup terminal", () => {
  assert.equal(
    observesTypedCleanupTerminal('{"event":"dev_frontend_ready"}'),
    false,
    "frontend readiness is not a bootstrap failure terminal",
  );
  assert.equal(
    observesTypedCleanupTerminal(
      '{"event":"dev_environment_cleanup_completed"}',
    ),
    true,
  );
});
