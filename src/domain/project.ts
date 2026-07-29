import type { EditorProjection } from "./generated/EditorProjection";
import type { ExportResult } from "./generated/ExportResult";
import type { ProjectIntent } from "./generated/ProjectIntent";

export type { AlbumSnapshot } from "./generated/AlbumSnapshot";
export type { ComposedFrame } from "./generated/ComposedFrame";
export type { ComposedPhoto } from "./generated/ComposedPhoto";
export type { ComposedSheet } from "./generated/ComposedSheet";
export type { CompositionPlan } from "./generated/CompositionPlan";
export type { EditorProjection } from "./generated/EditorProjection";
export type { EditorState } from "./generated/EditorState";
export type { ExportResult } from "./generated/ExportResult";
export type { FrameSnapshot } from "./generated/FrameSnapshot";
export type { Matrix2 } from "./generated/Matrix2";
export type { MediaCatalogItem } from "./generated/MediaCatalogItem";
export type { MediaTransform } from "./generated/MediaTransform";
export type { NumberRange } from "./generated/NumberRange";
export type { PhotoPlacement } from "./generated/PhotoPlacement";
export type { PhotoPlacementPlan } from "./generated/PhotoPlacementPlan";
export type { PhotoSnapshot } from "./generated/PhotoSnapshot";
export type { ProjectIntent } from "./generated/ProjectIntent";
export type { RectUm } from "./generated/RectUm";
export type { SheetRole } from "./generated/SheetRole";
export type { SheetSnapshot } from "./generated/SheetSnapshot";
export type { SizeUm as Size2 } from "./generated/SizeUm";
export type { VectorUm as Vector2 } from "./generated/VectorUm";

export interface ProjectBridge {
  load(): Promise<EditorProjection>;
  apply(intent: ProjectIntent): Promise<EditorProjection>;
  undo(): Promise<EditorProjection>;
  redo(): Promise<EditorProjection>;
  exportPreview(): Promise<ExportResult>;
}
