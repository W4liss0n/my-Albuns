import type {
  ProjectedBackgroundContent,
  ProjectedFrameBorder,
  ProjectedOverlayContent,
  ProjectedVisualDefaults,
} from "../domain/project";
import {
  applyScopedValue,
  type VisualScope,
} from "../application/scopedValues";

export type AlbumDesignScope = VisualScope;

export function setAlbumBackground(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
  content: ProjectedBackgroundContent,
): ProjectedVisualDefaults {
  return {
    ...defaults,
    background: applyScopedValue(defaults.background, scope, content),
  };
}

export function setAlbumOverlay(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
  content: ProjectedOverlayContent | null,
): ProjectedVisualDefaults {
  return {
    ...defaults,
    overlay: applyScopedValue(defaults.overlay, scope, content),
  };
}

export function setAlbumFrameBorder(
  defaults: ProjectedVisualDefaults,
  frameBorder: ProjectedFrameBorder,
): ProjectedVisualDefaults {
  return { ...defaults, frameBorder };
}
