import { afterEach, expect, test, vi } from "vitest";

import { tauriProjectFileDialog } from "./tauriProjectFileDialog";

const open = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/plugin-dialog", () => ({ open }));

afterEach(() => {
  open.mockReset();
});

test("opens exactly one existing file through the native dialog", async () => {
  open.mockResolvedValue(String.raw`C:\Álbuns\Casamento.myalbuns`);

  await expect(tauriProjectFileDialog.openProjectFile()).resolves.toBe(
    String.raw`C:\Álbuns\Casamento.myalbuns`,
  );
  expect(open).toHaveBeenCalledWith({
    directory: false,
    multiple: false,
    title: "Abrir Projeto",
  });
});

test("preserves cancellation as an explicit null result", async () => {
  open.mockResolvedValue(null);

  await expect(tauriProjectFileDialog.openProjectFile()).resolves.toBeNull();
});
