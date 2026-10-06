// @vitest-environment node
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";

import type { ProjectFailureDialogRequest } from "../application/globalProjectPort";
import { tauriProjectFailureDialogPort } from "./tauriProjectFailureDialogPort";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const failure = {
  context: "projectOpening",
  error: { code: "not_found", message: "O arquivo do projeto não foi encontrado." },
} as ProjectFailureDialogRequest;

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

test("asks the native host to present the failure with its context", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);

  await expect(
    tauriProjectFailureDialogPort.present(failure),
  ).resolves.toBeUndefined();

  expect(invoke).toHaveBeenCalledExactlyOnceWith(
    "show_project_failure_dialog",
    { context: failure.context, error: failure.error },
  );
});

test("settles normally when the native host cannot present the failure", async () => {
  vi.mocked(invoke).mockRejectedValue(new Error("dialog window unavailable"));

  await expect(
    tauriProjectFailureDialogPort.present(failure),
  ).resolves.toBeUndefined();

  expect(invoke).toHaveBeenCalledOnce();
});
