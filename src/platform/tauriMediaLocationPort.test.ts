// @vitest-environment node
import { beforeEach, expect, test, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { tauriMediaLocationPort } from "./tauriMediaLocationPort";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => vi.mocked(invoke).mockReset());

test("sends the media ID and preserves the native message", async () => {
  vi.mocked(invoke).mockRejectedValueOnce("A imagem não foi encontrada.");
  await expect(tauriMediaLocationPort.showInFolder("media-1")).rejects.toThrow("A imagem não foi encontrada.");
  expect(invoke).toHaveBeenCalledExactlyOnceWith("show_media_in_folder", { mediaId: "media-1" });
});

test("replaces an unreadable failure with a generic message", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({ unexpected: true });
  await expect(tauriMediaLocationPort.showInFolder("media-1")).rejects.toThrow("Não foi possível abrir a pasta da imagem. Tente novamente.");
});
