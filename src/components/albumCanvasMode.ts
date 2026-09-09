import type { ComposedSheet } from "../domain/project";
import type { AlbumCanvasMode } from "./albumCanvasContract";

export type AlbumCanvasModePolicy =
  | {
      editingSheetId: null;
      isolatedSheetId: string | null;
      enablesContinuousNavigation: boolean;
      enablesPhotoTransform: true;
      masksBleed: true;
      showsFrameResizeHandles: false;
      showsSheetBar: true;
      showsTechnicalGuides: false;
    }
  | {
      editingSheetId: string;
      isolatedSheetId: string;
      enablesContinuousNavigation: false;
      enablesPhotoTransform: false;
      masksBleed: false;
      showsFrameResizeHandles: true;
      showsSheetBar: false;
      showsTechnicalGuides: true;
    };

const NORMAL_MODE_POLICY: AlbumCanvasModePolicy = {
  editingSheetId: null,
  isolatedSheetId: null,
  enablesContinuousNavigation: true,
  enablesPhotoTransform: true,
  masksBleed: true,
  showsFrameResizeHandles: false,
  showsSheetBar: true,
  showsTechnicalGuides: false,
};

export function albumCanvasModePolicy(
  mode: AlbumCanvasMode,
): AlbumCanvasModePolicy {
  if (mode.kind === "normal") return mode.isolatedSheetId
    ? { ...NORMAL_MODE_POLICY, isolatedSheetId: mode.isolatedSheetId, enablesContinuousNavigation: false }
    : NORMAL_MODE_POLICY;
  return {
    editingSheetId: mode.sheetId,
    isolatedSheetId: mode.sheetId,
    enablesContinuousNavigation: false,
    enablesPhotoTransform: false,
    masksBleed: false,
    showsFrameResizeHandles: true,
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
    (sheet) => sheet.sheetId === policy.isolatedSheetId,
  );
}
