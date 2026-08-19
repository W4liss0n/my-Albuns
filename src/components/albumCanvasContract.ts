import type { CompositionPlan } from "../domain/project";
import type { GraphicsDiagnostic } from "../application/graphics";
import type { MediaPreviewDemand } from "../application/projectPorts";
import type { ViewportState } from "../state/viewport";
import type { ContinuousCanvasLayout } from "./canvasGeometry";

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
  scale: number;
}

export interface CanvasTechnicalGuides {
  bleedUm: number;
  safetyUm: number;
}

export interface SheetBarMetadata {
  sheetId: string;
  pageNumbers: readonly number[];
}

export interface AlbumCanvasProps {
  projectId: string;
  composition: CompositionPlan;
  sheetBarMetadata: readonly SheetBarMetadata[];
  mediaPreviewUrls?: Readonly<Record<string, string>>;
  technicalGuides?: CanvasTechnicalGuides;
  continuousCanvasLayout: ContinuousCanvasLayout;
  selectedFrameId: string | null;
  focusedSheetId: string | null;
  centeredSheetId: string | null;
  viewport: ViewportState;
  photoZoomPreview?: PhotoZoomPreview | null;
  onSelectFrame(frameId: string | null): void;
  onFocusSheet(sheetId: string): void;
  onCenteredSheetChange(sheetId: string): void;
  onViewportChange(viewport: ViewportState): void;
  onTransformPreview(preview: PhotoTransformPreview | null): void;
  onTransformCommit(delta: PhotoTransformDelta): Promise<boolean>;
  onCanvasMetricsChange?(metrics: CanvasMetrics): void;
  onMediaDemandChange?(demand: MediaPreviewDemand): void;
  onGraphicsUnavailable?(diagnostic: GraphicsDiagnostic): void;
}
