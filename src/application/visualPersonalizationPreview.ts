import type { FrameBorderValue } from "./frameBorderEditor";
import type { ScopedValue, VisualScope } from "./scopedValues";

export type PreviewBackgroundContent =
  | { kind: "color"; rgb: string }
  | { kind: "image"; previewUrl: string };

export type PreviewOverlayContent = { kind: "image"; previewUrl: string } | null;

export interface VisualPersonalizationPreview {
  background: ScopedValue<PreviewBackgroundContent>;
  fixedScope: VisualScope;
  frameBorder: FrameBorderValue;
  overlay: ScopedValue<PreviewOverlayContent>;
}

export interface VisualPreviewGeometry {
  bleedUm: number;
  heightUm: number;
  safetyUm: number;
  widthUm: number;
}
