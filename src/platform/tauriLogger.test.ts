import { afterEach, expect, test, vi } from "vitest";

import frontendLogEventFixture from "../../tests/fixtures/frontend-log-event.json";
import type { LogEvent } from "../application/logging";
import { tauriLogger } from "./tauriLogger";

const invoke = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke }));

afterEach(() => {
  invoke.mockReset();
});

test("forwards a structured frontend event to the Tauri host", () => {
  invoke.mockResolvedValue(undefined);
  const event = {
    ...frontendLogEventFixture,
    level: "info",
  } satisfies LogEvent;

  tauriLogger.write(event);

  expect(invoke).toHaveBeenCalledWith("frontend_log", { event });
});

test("contains transport failures inside the logging adapter", async () => {
  invoke.mockRejectedValue(new Error("IPC unavailable"));
  const warning = vi.spyOn(console, "warn").mockImplementation(() => undefined);

  expect(() =>
    tauriLogger.write({
      level: "warn",
      component: "application",
      event: "project_load_failed",
      reason: "bridge_error",
    }),
  ).not.toThrow();
  await vi.waitFor(() => {
    expect(warning).toHaveBeenCalledWith(
      "Não foi possível encaminhar um evento de diagnóstico ao host Tauri.",
    );
  });

  warning.mockRestore();
});
