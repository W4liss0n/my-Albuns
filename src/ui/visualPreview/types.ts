import type { FrameBorderValue } from "../../application/frameBorderEditor";
import type { ScopedValue, VisualScope } from "../../application/scopedValues";

export type PreviewBackgroundContent =
  | { kind: "color"; rgb: string }
  | { kind: "image"; preview: DecorativePreview };

export type PreviewOverlayContent =
  | { kind: "image"; preview: DecorativePreview }
  | null;

export type DecorativePreview =
  | { state: "pending" }
  | { state: "ready"; url: string | null }
  | { state: "absent" }
  | { state: "unavailable"; url: string | null };

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
