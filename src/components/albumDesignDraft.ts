import type {
  ProjectedBackgroundContent,
  ProjectedFrameBorder,
  ProjectedOverlayContent,
  ProjectedVisualDefaults,
} from "../domain/project";
import {
  applyToScope,
  type PersonalizationScope,
} from "../global/application/newProjectPersonalization";

export type AlbumDesignScope = PersonalizationScope;

export function setAlbumBackground(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
  content: ProjectedBackgroundContent,
): ProjectedVisualDefaults {
  return {
    ...defaults,
    background: applyToScope(defaults.background, scope, content),
  };
}

export function setAlbumOverlay(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
  content: ProjectedOverlayContent | null,
): ProjectedVisualDefaults {
  return {
    ...defaults,
    overlay: applyToScope(defaults.overlay, scope, content),
  };
}

export function setAlbumFrameBorder(
  defaults: ProjectedVisualDefaults,
  frameBorder: ProjectedFrameBorder,
): ProjectedVisualDefaults {
  return { ...defaults, frameBorder };
}
