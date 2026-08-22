import assert from "node:assert/strict";
import test from "node:test";

import {
  assertCausalHandoffObserved,
  isCausalHandoffObserved,
  isOwnedHostForestObserved,
  observesLogEvent,
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

test("a causal event remains observable from supervisor output when the process log ends first", () => {
  assert.equal(
    observesLogEvent(
      [
        '{"event":"application_started"}',
        'process_role="global" event="global_exited_after_project_handoff"',
      ],
      "global_exited_after_project_handoff",
    ),
    true,
  );
});

test("a rendered Host forest requires one observed descendant inside the development forest", () => {
  assert.equal(
    isOwnedHostForestObserved({
      hostProcessId: 301,
      hostForest: [301],
      developmentForest: [300, 301, 302],
    }),
    false,
  );
  assert.equal(
    isOwnedHostForestObserved({
      hostProcessId: 301,
      hostForest: [301, 302],
      developmentForest: [300, 301, 302],
    }),
    true,
  );
  assert.equal(
    isOwnedHostForestObserved({
      hostProcessId: 301,
      hostForest: [301, 302],
      developmentForest: [300, 301],
    }),
    false,
  );
});
