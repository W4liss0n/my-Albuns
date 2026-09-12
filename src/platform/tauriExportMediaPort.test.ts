import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";
import { tauriExportMediaPort } from "./tauriExportMediaPort";
import { parseExportMediaProblems } from "./exportMediaContract";
import { representativeProjection } from "../test/projectFixtures";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: class { onmessage = () => undefined; } }));
beforeEach(() => vi.mocked(invoke).mockReset());
const selection = { sheetId: "sheet-1", sheetNumber: 1, projectName: "Projeto" };

test("recovery sends only the selected sheet and receives native folder results", async () => {
  const problems = [{ mediaId: "photo-1", fileName: "Foto.jpg", state: "absent" }];
  vi.mocked(invoke).mockResolvedValueOnce(problems).mockResolvedValueOnce({ projection: representativeProjection, problems: [], notes: [] });
  expect(await tauriExportMediaPort.inspect(selection)).toEqual(problems);
  expect(await tauriExportMediaPort.relink(selection, vi.fn())).toEqual({ projection: representativeProjection, problems: [], notes: [] });
  expect(invoke).toHaveBeenNthCalledWith(1, "inspect_export_media", { sheetId: "sheet-1" });
  expect(invoke).toHaveBeenNthCalledWith(2, "relink_export_media", { sheetId: "sheet-1", onProgress: expect.any(Object) });
});

test("invalid inspection payloads never unblock Export", async () => {
  for (const value of [null, {}, [{ mediaId: "photo-1", fileName: "Foto.jpg", state: "ready" }], [{ state: "absent" }]]) {
    expect(parseExportMediaProblems(value)).toBeNull();
    vi.mocked(invoke).mockResolvedValueOnce(value);
    await expect(tauriExportMediaPort.inspect(selection)).rejects.toThrow("resposta inválida");
  }
});
