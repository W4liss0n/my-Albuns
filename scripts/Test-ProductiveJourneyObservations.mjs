import assert from "node:assert/strict";
import test from "node:test";

import {
  assertCausalProjectHandoff,
  assertCorrelatedJourneyTerminals,
} from "./ProductiveJourneyObservations.mjs";

const completeJourney = [
  '{"event":"host_ready","process_id":101}',
  '{"event":"project_ui_ready","process_id":101}',
  '{"event":"global_exited_after_project_handoff","process_id":100}',
  '{"event":"imaging_process_stopped","process_id":102}',
  '{"event":"project_window_destroyed","process_id":101}',
].join("\n");

test("accepts only the observed causal handoff order", () => {
  assert.doesNotThrow(() => assertCausalProjectHandoff(completeJourney));

  assert.throws(
    () =>
      assertCausalProjectHandoff(
        [
          '{"event":"host_ready"}',
          '{"event":"global_exited_after_project_handoff"}',
        ].join("\n"),
      ),
    /project_ui_ready/,
  );
  assert.throws(
    () =>
      assertCausalProjectHandoff(
        [
          '{"event":"global_exited_after_project_handoff"}',
          '{"event":"host_ready"}',
          '{"event":"project_ui_ready"}',
        ].join("\n"),
      ),
    /causal order/,
  );
});

test("correlates one terminal to each bootstrap and imaging attempt", () => {
  const records = [
    { event: "host_ready", process_id: 201 },
    { event: "project_ui_ready", process_id: 201 },
    { event: "global_exited_after_project_handoff", process_id: 200 },
    { event: "host_ready", process_id: 203 },
    { event: "project_ui_ready", process_id: 203 },
    { event: "global_exited_after_project_handoff", process_id: 202 },
    {
      event: "imaging_process_spawned",
      process_id: 201,
      imaging_process_id: 204,
    },
    { event: "imaging_process_stopped", process_id: 204 },
  ];

  assert.deepEqual(
    assertCorrelatedJourneyTerminals(records, {
      bootstraps: [
        { globalProcessId: 200, hostProcessId: 201 },
        { globalProcessId: 202, hostProcessId: 203 },
      ],
      imagingAttempts: [{ hostProcessId: 201, imagingProcessId: 204 }],
    }),
    { bootstraps: 2, imagingAttempts: 1 },
  );

  assert.throws(
    () =>
      assertCorrelatedJourneyTerminals(
        [...records, { event: "project_ui_ready", process_id: 203 }],
        {
          bootstraps: [
            { globalProcessId: 200, hostProcessId: 201 },
            { globalProcessId: 202, hostProcessId: 203 },
          ],
          imagingAttempts: [
            { hostProcessId: 201, imagingProcessId: 204 },
          ],
        },
      ),
    /project_ui_ready.*203.*2/,
  );
});
