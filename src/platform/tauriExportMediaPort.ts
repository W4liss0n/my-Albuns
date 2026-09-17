import { invokeImageProcessing } from "./invokeImageProcessing";
import { invoke } from "@tauri-apps/api/core";
import type { ExportMediaPort } from "../application/exportMedia";
import type { ExportRelinkResult } from "./generated/ExportRelinkResult";
import { parseExportMediaProblems } from "./exportMediaContract";

export const tauriExportMediaPort: ExportMediaPort = {
  inspect: async ({ sheetId, options }) => {
    const problems = parseExportMediaProblems(await invoke("inspect_export_media", { sheetId, sheetIds: options.sheetIds }));
    if (!problems) throw new Error("A verificação dos Arquivos retornou uma resposta inválida.");
    return problems;
  },
  relink: ({ sheetId, options }, onProgress) => invokeImageProcessing<ExportRelinkResult>("relink_export_media", { sheetId, sheetIds: options.sheetIds }, onProgress),
};
