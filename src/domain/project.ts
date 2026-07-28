import { invoke } from "@tauri-apps/api/core";

export interface RectUm {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface MediaTransform {
  panX: number;
  panY: number;
  userZoom: number;
  quarterTurns: number;
  fineRotationDegrees: number;
  mirrorX: boolean;
}

export interface PhotoSnapshot {
  mediaId: string;
  name: string;
  sourceWidthPx: number;
  sourceHeightPx: number;
  palette: [string, string, string];
  transform: MediaTransform;
}

export interface FrameSnapshot {
  id: string;
  rect: RectUm;
  zIndex: number;
  photo: PhotoSnapshot | null;
}

export type SheetRole = "initial" | "internal" | "final";

export interface SheetSnapshot {
  id: string;
  number: number;
  role: SheetRole;
  widthUm: number;
  heightUm: number;
  frames: FrameSnapshot[];
  hasOverlay: boolean;
}

export interface MediaCatalogItem {
  id: string;
  name: string;
  palette: [string, string, string];
  usageCount: number;
}

export interface EditorState {
  projectId: string;
  projectName: string;
  album: {
    sheets: SheetSnapshot[];
    media: MediaCatalogItem[];
  };
  revision: number;
  savedRevision: number;
  dirty: boolean;
  canUndo: boolean;
  canRedo: boolean;
}

export interface ComposedPhoto {
  mediaId: string;
  name: string;
  drawRect: RectUm;
  rotationDegrees: number;
  mirrorX: boolean;
  palette: [string, string, string];
}

export interface ComposedFrame {
  frameId: string;
  clipRect: RectUm;
  zIndex: number;
  photo: ComposedPhoto | null;
}

export interface ComposedSheet {
  sheetId: string;
  number: number;
  widthUm: number;
  heightUm: number;
  hasOverlay: boolean;
  frames: ComposedFrame[];
}

export interface CompositionPlan {
  sheets: ComposedSheet[];
}

export interface EditorProjection {
  state: EditorState;
  composition: CompositionPlan;
}

export type ProjectIntent =
  | {
      kind: "panPhoto";
      frameId: string;
      deltaX: number;
      deltaY: number;
    }
  | {
      kind: "fillLeftmostPlaceholder";
      sheetId: string;
      mediaId: string;
    }
  | {
      kind: "zoomPhoto";
      frameId: string;
      delta: number;
    };

export interface ExportResult {
  outputPath: string;
  widthPx: number;
  heightPx: number;
}

export interface ProjectBridge {
  load(): Promise<EditorProjection>;
  apply(intent: ProjectIntent): Promise<EditorProjection>;
  undo(): Promise<EditorProjection>;
  redo(): Promise<EditorProjection>;
  exportPreview(): Promise<ExportResult>;
}

export const tauriProjectBridge: ProjectBridge = {
  load: () => invoke<EditorProjection>("project_state"),
  apply: (intent) =>
    invoke<EditorProjection>("apply_project_intent", { intent }),
  undo: () => invoke<EditorProjection>("undo_project"),
  redo: () => invoke<EditorProjection>("redo_project"),
  exportPreview: () => invoke<ExportResult>("export_spike"),
};
