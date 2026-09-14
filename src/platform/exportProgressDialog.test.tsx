import { act, fireEvent, render, screen } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, expect, test, vi } from "vitest";

import { ExportPreviewControl } from "../components/ExportPreviewControl";
import { createExportHarness } from "../test/exportPipelineHarness";
import { createTauriProjectDialogPort } from "./tauriProjectDialogPort";
import { parseProjectDialogState } from "./projectDialogContract";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(listen).mockReset();
});

test.each([
  ["loading_sources", 14],
  ["composing", 3],
  ["publishing", 18],
] as const)("keeps the native export dialog open for %s progress with %i units", async (stage, totalUnits) => {
  const harness = createExportHarness();
  const rejectedStates: unknown[] = [];
  vi.mocked(listen).mockResolvedValue(() => undefined);
  const native = vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === "present_project_dialog" &&
        !parseProjectDialogState((args as { state: unknown }).state)) {
      rejectedStates.push((args as { state: unknown }).state);
      throw new Error("Invalid native dialog payload");
    }
  });
  render(<ExportPreviewControl dialogPort={createTauriProjectDialogPort()}
    exportPipelinePort={harness.port} projectId="project-a"
    selection={{ projectName: "Album", sheetId: "first", sheetNumber: 1 }} />);
  fireEvent.click(screen.getByRole("button", { name: "Exportar" }));
  await act(async () => harness.attempts[0].emit({ event: "started", cancellable: true }));
  native.mockClear();

  // Fractional pipeline percentages must remain integer values at the native dialog boundary.
  for (let completedUnits = 0; completedUnits <= totalUnits; completedUnits++) {
    await act(async () => harness.attempts[0].emit({ event: "progress", stage, overallPercent: 100 * completedUnits / totalUnits,
      units: { kind: "measured", completedUnits, totalUnits }, cancellable: stage !== "publishing" }));
  }

  expect(native.mock.calls.some(([command]) => command === "dismiss_project_dialog"), JSON.stringify(rejectedStates)).toBe(false);
  expect(harness.attempts[0].cancel).not.toHaveBeenCalled();
  expect(native).toHaveBeenLastCalledWith("present_project_dialog", expect.objectContaining({
    state: expect.objectContaining({ kind: "exportProgress" }),
  }));
});

