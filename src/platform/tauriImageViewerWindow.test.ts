import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import { tauriImageViewerWindow } from "./tauriImageViewerWindow";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

const request = { sessionId: "viewer-1", targetMediaId: "target", referenceMediaId: "reference", targetFace: [], referenceFace: [] };
beforeEach(() => { vi.mocked(invoke).mockReset(); });

test("preparation preserves the native rejection reason as an Error for the photo tooltip", async () => {
  const reason = "Os olhos da referência precisam estar visivelmente mais abertos.";
  vi.mocked(invoke).mockRejectedValue(reason);
  await expect(tauriImageViewerWindow.prepareCorrection!(request)).rejects.toEqual(new Error(reason));
});

test("applying preserves the native rejection reason for a recoverable preview", async () => {
  const reason = "A prévia da correção expirou.";
  vi.mocked(invoke).mockRejectedValue(reason);
  await expect(tauriImageViewerWindow.applyCorrection!("viewer-1", "token")).rejects.toEqual(new Error(reason));
});

test("unrecognized native errors use the action-specific plain-language fallback", async () => {
  vi.mocked(invoke).mockRejectedValue({ internal: "details" });
  await expect(tauriImageViewerWindow.prepareCorrection!(request)).rejects.toEqual(new Error("Não foi possível corrigir os olhos."));
  await expect(tauriImageViewerWindow.applyCorrection!("viewer-1", "token")).rejects.toEqual(new Error("Não foi possível aplicar a correção."));
});

test("preparation forwards the selected faces and returns the native preview unchanged", async () => {
  const preview = { token: "token", url: "preview://corrected" };
  vi.mocked(invoke).mockResolvedValue(preview);
  await expect(tauriImageViewerWindow.prepareCorrection!(request)).resolves.toBe(preview);
  expect(invoke).toHaveBeenCalledWith("prepare_eye_correction", request);
});
