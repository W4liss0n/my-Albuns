import { expect, test, vi } from "vitest";

const coreApi = vi.hoisted(() => ({
  invoke: vi.fn(async () => undefined),
}));

vi.mock("@tauri-apps/api/core", () => coreApi);

import { resolveOpeningRecovery } from "./tauriOpeningDialogControls";

test("resolves only the correlated external opening Recovery attempt", async () => {
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
