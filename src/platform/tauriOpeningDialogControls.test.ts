import { expect, test, vi } from "vitest";

const coreApi = vi.hoisted(() => ({
  invoke: vi.fn(async () => undefined),
}));

vi.mock("@tauri-apps/api/core", () => coreApi);

import {
  resolveOpeningExternalCopy,
  resolveOpeningRecovery,
} from "./tauriOpeningDialogControls";

test("resolves only the correlated external-copy opening attempt", async () => {
  await resolveOpeningExternalCopy("external-attempt-5", "saveCopyAs");

  expect(coreApi.invoke).toHaveBeenCalledOnce();
  expect(coreApi.invoke).toHaveBeenCalledWith(
    "resolve_opening_external_copy",
    {
      attemptId: "external-attempt-5",
      decision: "saveCopyAs",
    },
  );
});

test("resolves only the correlated external opening Recovery attempt", async () => {
  coreApi.invoke.mockClear();
  await resolveOpeningRecovery(
    "attempt-17",
    "discardCheckpointAndOpenLastSaved",
  );

  expect(coreApi.invoke).toHaveBeenCalledOnce();
  expect(coreApi.invoke).toHaveBeenCalledWith("resolve_opening_recovery", {
    attemptId: "attempt-17",
    decision: "discardCheckpointAndOpenLastSaved",
  });
});
