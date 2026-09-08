import type {
  CompositionPlan,
  ComposedFrame,
  FrameGeometryEdit,
  PhotoDropTarget,
} from "../domain/project";
import type { GraphicsDiagnostic } from "../application/graphics";
import type { MediaPreviewDemand } from "../application/projectPorts";
import type { PointerDragThreshold } from "../application/projectPorts";
import type { ViewportState } from "../state/viewport";
import type { ContinuousCanvasLayout } from "./canvasGeometry";
import type {
  SheetReorderRepresentation,
  SheetReorderStatus,
} from "./sheetReorderSession";

export interface PhotoZoomPreview {
  frameId: string;
  value: number;
}

export interface PhotoTransformPreview {
  frameId: string;
  panX: number;
  panY: number;
  zoom: number;
}

export interface PhotoTransformDelta {
  frameId: string;
  deltaPanX: number;
  deltaPanY: number;
  deltaZoom: number;
}

export interface CanvasMetrics {
  width: number;
  /** Dimensionless scene scale; Canvas coordinates use 1 unit per 1,000 µm. */
  scale: number;
}

export interface CanvasTechnicalGuides {
  bleedUm: number;
  safetyUm: number;
}

export interface SheetBarMetadata {
  sheetId: string;
  pageNumbers: readonly number[];
  layoutLocked: boolean;
}

export interface CanvasSheetReorder {
  disabled: boolean;
  representation: SheetReorderRepresentation;
  status: SheetReorderStatus;
  onPreview(draggedSheetId: string, targetIndex: number): void;
  onDrop(): void;
  onCancel(): void;
  onSelect(sheetId: string): void;
}

export type AlbumCanvasMode =
  | { kind: "normal" }
  | { kind: "sheet-editing"; sheetId: string };

export interface CanvasPhotoDropPoint {
  sheetId: string;
  xUm: number;
  yUm: number;
}

export interface CanvasFrameGeometry {
  disabled: boolean;
  dragThreshold: PointerDragThreshold | null;
  preview(edit: FrameGeometryEdit): Promise<ComposedFrame[]>;
  /** The exact committed composition bridges command completion and React presentation. */
  commit(edit: FrameGeometryEdit): Promise<ComposedFrame[] | null>;
  onError(message: string): void;
}
export interface CanvasFrameContentSwap {
  disabled: boolean;
  dragThreshold: PointerDragThreshold | null;
  resolveTarget(point: CanvasPhotoDropPoint): Promise<PhotoDropTarget>;
  /** Resolves the release point and swaps in the shared mutation queue. */
  commit(sourceFrameId: string, point: CanvasPhotoDropPoint): Promise<boolean>;
  onError(message: string): void;
}
export interface AlbumCanvasProps {
  projectId: string;
  mode: AlbumCanvasMode;
  composition: CompositionPlan;
  sheetBarMetadata: readonly SheetBarMetadata[];
  mediaPreviewUrls?: Readonly<Record<string, string>>;
  technicalGuides?: CanvasTechnicalGuides;
  continuousCanvasLayout: ContinuousCanvasLayout;
  selectedFrameIds: readonly string[];
  focusedSheetId: string | null;
  centeredSheetId: string | null;
  viewport: ViewportState;
  draggedPhotoId?: string | null;
  sheetReorder?: CanvasSheetReorder;
  photoDropHighlight?: PhotoDropTarget | null;
  photoZoomPreview?: PhotoZoomPreview | null;
  frameGeometry?: CanvasFrameGeometry;
  frameContentSwap?: CanvasFrameContentSwap;
  onSelectFrame(frameId: string | null, toggle?: boolean): void;
  onEditSheet(sheetId: string): void;
  onFocusSheet(sheetId: string): void;
  onCenteredSheetChange(sheetId: string): void;
  onViewportChange(viewport: ViewportState): void;
  onTransformPreview(preview: PhotoTransformPreview | null): void;
  onTransformCommit(delta: PhotoTransformDelta): Promise<boolean>;
  onResolvePhotoDropTarget?(
    mediaId: string,
    point: CanvasPhotoDropPoint,
  ): Promise<PhotoDropTarget>;
  onDropPhoto?(
    mediaId: string,
    point: CanvasPhotoDropPoint,
  ): Promise<boolean>;
  onPhotoDragCancel?(): void;
  onOpenFrameContextMenu?(frameId: string, position: { x: number; y: number }): void;
  onOpenEmptyCanvasContextMenu?(sheetId: string, position: { x: number; y: number }): void;
  onOpenSheetContextMenu?(
    sheetId: string,
    position: { x: number; y: number },
  ): void;
  onCanvasMetricsChange?(metrics: CanvasMetrics): void;
  onMediaDemandChange?(demand: MediaPreviewDemand): void;
  onGraphicsUnavailable?(diagnostic: GraphicsDiagnostic): void;
}
