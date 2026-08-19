import type { ComposedSheet } from "../domain/project";
import type { AlbumCanvasMode } from "./albumCanvasContract";

export type AlbumCanvasModePolicy =
  | {
      editingSheetId: null;
      enablesContinuousNavigation: true;
      enablesPhotoTransform: true;
      masksBleed: true;
      showsSheetBar: true;
      showsTechnicalGuides: false;
    }
  | {
      editingSheetId: string;
      enablesContinuousNavigation: false;
      enablesPhotoTransform: false;
      masksBleed: false;
      showsSheetBar: false;
      showsTechnicalGuides: true;
    };

const NORMAL_MODE_POLICY: AlbumCanvasModePolicy = {
  editingSheetId: null,
  enablesContinuousNavigation: true,
  enablesPhotoTransform: true,
  masksBleed: true,
  showsSheetBar: true,
  showsTechnicalGuides: false,
};

export function albumCanvasModePolicy(
  mode: AlbumCanvasMode,
): AlbumCanvasModePolicy {
  if (mode.kind === "normal") return NORMAL_MODE_POLICY;
  return {
    editingSheetId: mode.sheetId,
    enablesContinuousNavigation: false,
    enablesPhotoTransform: false,
    masksBleed: false,
    showsSheetBar: false,
    showsTechnicalGuides: true,
  };
}

export function sheetsForCanvasMode(
  sheets: readonly ComposedSheet[],
  policy: AlbumCanvasModePolicy,
) {
  if (policy.enablesContinuousNavigation) return sheets;
  return sheets.filter(
    (sheet) => sheet.sheetId === policy.editingSheetId,
  );
}
