import {
  Container,
  type FederatedPointerEvent,
  type FederatedWheelEvent,
  Graphics,
  Rectangle,
  Sprite,
  type FillGradient,
  type Text,
  type Texture,
} from "pixi.js";

import type {
  ComposedSheet,
  NormalizedPan,
  ProjectedFrameBorder,
} from "../domain/project";
import type {
  CanvasTechnicalGuides,
  SheetBarMetadata,
} from "./albumCanvasContract";
import type { AlbumCanvasModePolicy } from "./albumCanvasMode";
import {
  createCanvasSheetPresentation,
  MICROMETER_TO_CANVAS_PIXEL,
} from "./canvasGeometry";
import {
  createCanvasSheetViewGeometry,
  type CanvasBounds,
} from "./canvasSheetViewGeometry";
import { pixiColor } from "./pixiColor";
import {
  createPhotoGeometry,
  type CanvasPhotoPlacement,
  type CanvasPoint,
  type PhotoGeometry,
} from "./photoGeometry";
import {
  createFrameSelectionRenderNode,
  type FrameSelectionRenderNode,
} from "./frameSelectionRenderNode";
import {
  frameOutlineStyle,
  photoPaletteIndexForStripe,
  SHEET_VISUAL_STYLE,
} from "./sheetVisualStyle";
import {
  createSheetBarRenderNode,
  setSheetBarSheetHovered,
  stopSheetBarTransition,
  type SheetBarRenderNode,
} from "./sheetBarRenderNode";
import {
  createCanvasFramePlaceholder,
  createSheetBleedMask,
  createSheetCenterLine,
  createSheetInactiveSide,
  createSheetSurfaceRenderNodes,
  createSheetTechnicalGuideNodes,
} from "./sheetSurfaceRenderNodes";

const PAN_OUTSIDE_OPACITY = 0.24;

export interface PhotoRenderNode {
  frameId: string;
  layer: Container;
  outsideLayer: Container;
  thirdsGuides: Graphics;
  geometry: PhotoGeometry;
  baseZoom: number;
  baseScaleX: number;
  originalX: number;
  originalY: number;
  pan: NormalizedPan;
}

export interface SheetRenderNode {
  container: Container;
  signature: string;
  photoNodes: PhotoRenderNode[];
  placeholderLabels: Text[];
  inactiveSideGradient: FillGradient | null;
  frameSelections: Map<string, FrameSelectionRenderNode>;
  frameDropOutlines: Map<string, Graphics>;
  focusOutline: Graphics;
  sheetDropOutline: Graphics;
  sheetBar: SheetBarRenderNode;
  viewBounds: CanvasBounds;
  activeOffsetXPx: number;
}

interface SheetRenderNodeCallbacks {
  previewTextureFor: (mediaId: string) => Texture | undefined;
  onSheetTap: (sheetId: string) => void;
  onSheetDoubleTap: (sheetId: string) => void;
  onFrameTap: (sheetId: string, frameId: string) => void;
  onPhotoPanStart: (
    photoNode: PhotoRenderNode,
    event: FederatedPointerEvent,
  ) => void;
  onPhotoWheel: (
    photoNode: PhotoRenderNode,
    event: FederatedWheelEvent,
  ) => void;
}

interface PhotoPreviewLayerOptions {
  label: string;
  drawWidth: number;
  drawHeight: number;
  center: CanvasPoint;
  rotationDegrees: number;
  mirrorX: boolean;
  palette: readonly string[];
  previewTexture?: Texture;
}

export function createSheetRenderNode(
  sheet: ComposedSheet,
  sheetBarMetadata: SheetBarMetadata | undefined,
  frameBorder: ProjectedFrameBorder,
  technicalGuides: CanvasTechnicalGuides | undefined,
  modePolicy: AlbumCanvasModePolicy,
  signature: string,
  callbacks: SheetRenderNodeCallbacks,
): SheetRenderNode {
  const sheetContainer = new Container();
  const presentation = createCanvasSheetPresentation(sheet);
  const viewGeometry = createCanvasSheetViewGeometry(
    sheet,
    presentation,
    technicalGuides?.bleedUm,
    modePolicy.masksBleed,
  );
  const dispatchSheetDoubleTap = (event: FederatedPointerEvent) => {
    if (!(event.detail >= 2)) return false;
    callbacks.onSheetDoubleTap(sheet.sheetId);
    return true;
  };
  sheetContainer.label = `canvas-sheet-${sheet.sheetId}`;
  sheetContainer.eventMode = "static";
  sheetContainer.hitArea = new Rectangle(
    viewGeometry.visibleOuterBounds.x,
    viewGeometry.visibleOuterBounds.y,
    viewGeometry.visibleOuterBounds.width,
    viewGeometry.visibleOuterBounds.height,
  );
  sheetContainer.cursor = "default";
  sheetContainer.on("pointertap", (event: FederatedPointerEvent) => {
    if (event.button !== 0) return;
    if (event.target === sheetContainer) {
      if (!dispatchSheetDoubleTap(event)) {
        callbacks.onSheetTap(sheet.sheetId);
      }
    }
  });

  sheetContainer.addChild(
    ...createSheetSurfaceRenderNodes(
      sheet,
      viewGeometry.visibleOuterBounds,
    ),
  );
  const inactiveSide = createSheetInactiveSide(
    sheet,
    viewGeometry.inactiveSideBounds,
  );
  if (inactiveSide) sheetContainer.addChild(inactiveSide.container);

  const activeContent = new Container();
  activeContent.label = `sheet-active-content-${sheet.sheetId}`;
  activeContent.eventMode = "passive";
  activeContent.position.set(presentation.activeOffsetXPx, 0);
  sheetContainer.addChild(activeContent);

  for (const background of sheet.backgrounds) {
    const x = background.drawRect.x * MICROMETER_TO_CANVAS_PIXEL;
    const y = background.drawRect.y * MICROMETER_TO_CANVAS_PIXEL;
    const backgroundWidth =
      background.drawRect.width * MICROMETER_TO_CANVAS_PIXEL;
    const backgroundHeight =
      background.drawRect.height * MICROMETER_TO_CANVAS_PIXEL;
    if (background.kind === "color") {
      const color = new Graphics()
        .rect(x, y, backgroundWidth, backgroundHeight)
        .fill({ color: pixiColor(background.rgb) });
      color.label = `background-color-${background.rgb}`;
      color.eventMode = "none";
      activeContent.addChild(color);
      continue;
    }
    const previewTexture = callbacks.previewTextureFor(background.mediaId);
    if (previewTexture) {
      const sprite = new Sprite({ texture: previewTexture });
      sprite.label = `background-media-${background.mediaId}`;
      sprite.position.set(x, y);
      sprite.width = backgroundWidth;
      sprite.height = backgroundHeight;
      sprite.eventMode = "none";
      activeContent.addChild(sprite);
    } else {
      const fallback = new Graphics()
        .rect(x, y, backgroundWidth, backgroundHeight)
        .fill({ color: pixiColor(SHEET_VISUAL_STYLE.mediaFallback.fill) });
      fallback.label = `background-media-fallback-${background.mediaId}`;
      fallback.eventMode = "none";
      activeContent.addChild(fallback);
    }
  }

  const centerLine = createSheetCenterLine(
    sheet,
    presentation,
    viewGeometry.visibleOuterBounds,
  );
  if (centerLine) sheetContainer.addChild(centerLine);

  const frameSelections = new Map<string, FrameSelectionRenderNode>();
  const frameDropOutlines = new Map<string, Graphics>();
  const frameSelectionLayer = new Container();
  frameSelectionLayer.label = `frame-selection-layer-${sheet.sheetId}`;
  frameSelectionLayer.eventMode = "none";
  const photoNodes: PhotoRenderNode[] = [];
  const placeholderLabels: Text[] = [];
  for (const frame of sheet.frames) {
    const frameContainer = new Container();
    const frameX = frame.clipRect.x * MICROMETER_TO_CANVAS_PIXEL;
    const frameY = frame.clipRect.y * MICROMETER_TO_CANVAS_PIXEL;
    const frameWidth = frame.clipRect.width * MICROMETER_TO_CANVAS_PIXEL;
    const frameHeight = frame.clipRect.height * MICROMETER_TO_CANVAS_PIXEL;
    frameContainer.label = `canvas-frame-${frame.frameId}`;
    frameContainer.position.set(frameX, frameY);
    frameContainer.eventMode = "static";
    frameContainer.hitArea = new Rectangle(0, 0, frameWidth, frameHeight);
    frameContainer.cursor = "default";

    let photoNode: PhotoRenderNode | null = null;
    let emptyPlaceholder: ReturnType<
      typeof createCanvasFramePlaceholder
    > | null = null;
    if (frame.photo) {
      const geometry = createPhotoGeometry(
        frame.photo.placement,
        MICROMETER_TO_CANVAS_PIXEL,
      );
      const previewOptions = {
        drawWidth: geometry.current.size.width,
        drawHeight: geometry.current.size.height,
        center: geometry.current.center,
        rotationDegrees: frame.photo.rotationDegrees,
        mirrorX: frame.photo.mirrorX,
        palette: frame.photo.palette,
        previewTexture: callbacks.previewTextureFor(frame.photo.mediaId),
      };
      const outsidePhotoLayer = createPhotoPreviewLayer({
        ...previewOptions,
        label: "photo-pan-outside-preview",
      });
      outsidePhotoLayer.alpha = PAN_OUTSIDE_OPACITY;
      outsidePhotoLayer.eventMode = "none";
      outsidePhotoLayer.visible = false;
      const photoLayer = createPhotoPreviewLayer({
        ...previewOptions,
        label: "photo-pan-inside-preview",
      });
      const clip = new Graphics()
        .rect(0, 0, frameWidth, frameHeight)
        .fill(0xffffff);
      const photoViewport = new Container();
      photoViewport.addChild(photoLayer);
      photoViewport.mask = clip;
      const thirdsGuides = createThirdsGuides(frameWidth, frameHeight);
      frameContainer.addChild(
        outsidePhotoLayer,
        photoViewport,
        clip,
        thirdsGuides,
      );

      const baseZoom = frame.photo.placement.currentZoom;
      photoNode = {
        frameId: frame.frameId,
        layer: photoLayer,
        outsideLayer: outsidePhotoLayer,
        thirdsGuides,
        geometry,
        baseZoom,
        baseScaleX: frame.photo.mirrorX ? -1 : 1,
        originalX: photoLayer.x,
        originalY: photoLayer.y,
        pan: frame.photo.placement.currentPan,
      };
      photoNodes.push(photoNode);
    } else {
      emptyPlaceholder = createCanvasFramePlaceholder(
        frame.frameId,
        frameWidth,
        frameHeight,
      );
      frameContainer.addChild(emptyPlaceholder.container);
      placeholderLabels.push(emptyPlaceholder.label);
    }

    const outlineStyle = frameOutlineStyle(frame.photo !== null);
    const outline = new Graphics()
      .rect(0, 0, frameWidth, frameHeight)
      .stroke({
        color: pixiColor(outlineStyle.outline),
        width: outlineStyle.outlineWidthPx,
        alpha: outlineStyle.outlineOpacity,
        pixelLine: !frame.photo,
      });
    outline.label = `frame-outline-${frame.frameId}`;
    outline.eventMode = "none";
    let persistedBorder: Graphics | null = null;
    if (frameBorder.kind === "solid" && frame.borderFillRects.length > 0) {
      persistedBorder = new Graphics();
      for (const rect of frame.borderFillRects) {
        persistedBorder.rect(
          (rect.x - frame.clipRect.x) * MICROMETER_TO_CANVAS_PIXEL,
          (rect.y - frame.clipRect.y) * MICROMETER_TO_CANVAS_PIXEL,
          rect.width * MICROMETER_TO_CANVAS_PIXEL,
          rect.height * MICROMETER_TO_CANVAS_PIXEL,
        );
      }
      persistedBorder.fill({
        color: pixiColor(frameBorder.rgb),
        alpha: 1,
      });
    }
    if (persistedBorder) {
      persistedBorder.label = `frame-persisted-border-${frame.frameId}`;
      persistedBorder.eventMode = "none";
    }
    frameContainer.addChild(
      outline,
      ...(persistedBorder ? [persistedBorder] : []),
    );

    const frameSelection = createFrameSelectionRenderNode(
      frame.frameId,
      frameWidth,
      frameHeight,
      modePolicy.showsFrameResizeHandles &&
        sheetBarMetadata?.layoutLocked === false,
    );
    frameSelection.container.position.set(frameX, frameY);
    frameSelections.set(frame.frameId, frameSelection);
    frameSelectionLayer.addChild(frameSelection.container);

    const frameDropOutline = new Graphics()
      .rect(frameX, frameY, frameWidth, frameHeight)
      .stroke({
        ...SHEET_VISUAL_STYLE.technicalOutlineStroke,
        width: 2,
      });
    frameDropOutline.label = `frame-photo-drop-${frame.frameId}`;
    frameDropOutline.eventMode = "none";
    frameDropOutline.visible = false;
    frameDropOutlines.set(frame.frameId, frameDropOutline);
    frameSelectionLayer.addChild(frameDropOutline);

    frameContainer.on("pointertap", (event: FederatedPointerEvent) => {
      if (event.button !== 0) return;
      event.stopPropagation();
      if (dispatchSheetDoubleTap(event)) return;
      if (!event.altKey) {
        callbacks.onFrameTap(sheet.sheetId, frame.frameId);
      }
    });
    frameContainer.on("pointerdown", (event: FederatedPointerEvent) => {
      if (!modePolicy.enablesPhotoTransform || !event.altKey || !photoNode) {
        return;
      }
      event.stopPropagation();
      callbacks.onPhotoPanStart(photoNode, event);
    });
    frameContainer.on("wheel", (event: FederatedWheelEvent) => {
      if (!modePolicy.enablesPhotoTransform || !event.altKey || !photoNode) {
        return;
      }
      callbacks.onPhotoWheel(photoNode, event);
    });
    activeContent.addChild(frameContainer);
  }

  for (const composedOverlay of sheet.overlays) {
    const previewTexture = callbacks.previewTextureFor(
      composedOverlay.mediaId,
    );
    if (previewTexture) {
      const overlay = new Sprite({ texture: previewTexture });
      overlay.label = `decorative-overlay-${composedOverlay.mediaId}`;
      overlay.position.set(
        composedOverlay.drawRect.x * MICROMETER_TO_CANVAS_PIXEL,
        composedOverlay.drawRect.y * MICROMETER_TO_CANVAS_PIXEL,
      );
      overlay.width =
        composedOverlay.drawRect.width * MICROMETER_TO_CANVAS_PIXEL;
      overlay.height =
        composedOverlay.drawRect.height * MICROMETER_TO_CANVAS_PIXEL;
      overlay.eventMode = "none";
      activeContent.addChild(overlay);
    } else {
      const overlayStyle = SHEET_VISUAL_STYLE.overlay;
      const overlay = new Graphics()
        .roundRect(
          composedOverlay.drawRect.x * MICROMETER_TO_CANVAS_PIXEL,
          composedOverlay.drawRect.y * MICROMETER_TO_CANVAS_PIXEL,
          composedOverlay.drawRect.width * MICROMETER_TO_CANVAS_PIXEL,
          composedOverlay.drawRect.height * MICROMETER_TO_CANVAS_PIXEL,
          overlayStyle.cornerRadiusPx,
        )
        .stroke({
          color: pixiColor(overlayStyle.outline),
          width: overlayStyle.outlineWidthPx,
          alpha: overlayStyle.outlineOpacity,
        });
      overlay.label = `decorative-overlay-fallback-${composedOverlay.mediaId}`;
      overlay.eventMode = "none";
      activeContent.addChild(overlay);
    }
  }

  if (
    modePolicy.masksBleed &&
    (technicalGuides?.bleedUm ?? 0) > 0
  ) {
    const bleedMask = createSheetBleedMask(
      sheet,
      viewGeometry.activeBounds,
    );
    activeContent.mask = bleedMask;
    sheetContainer.addChild(bleedMask);
  }
  if (modePolicy.showsTechnicalGuides) {
    for (const guide of createSheetTechnicalGuideNodes(
      sheet,
      technicalGuides,
    )) {
      activeContent.addChild(guide);
    }
  }
  activeContent.addChild(frameSelectionLayer);

  const sheetBar = createSheetBarRenderNode(
    sheet,
    sheetBarMetadata,
    presentation,
    viewGeometry,
  );
  sheetBar.container.visible = modePolicy.showsSheetBar;
  sheetBar.container.on(
    "pointertap",
    (event: FederatedPointerEvent) => {
      if (event.button !== 0) return;
      if (event.target === sheetBar.container) {
        if (!dispatchSheetDoubleTap(event)) {
          callbacks.onSheetTap(sheet.sheetId);
        }
      }
    },
  );
  sheetContainer.on("pointerenter", () => {
    setSheetBarSheetHovered(sheetBar, true);
  });
  sheetContainer.on("pointerleave", () => {
    setSheetBarSheetHovered(sheetBar, false);
  });
  sheetContainer.addChild(sheetBar.container);

  const focusOutline = new Graphics()
    .rect(
      viewGeometry.visibleOuterBounds.x,
      viewGeometry.visibleOuterBounds.y,
      viewGeometry.visibleOuterBounds.width,
      viewGeometry.visibleOuterBounds.height,
    )
    .stroke(SHEET_VISUAL_STYLE.technicalOutlineStroke);
  focusOutline.label = `sheet-focus-${sheet.sheetId}`;
  focusOutline.eventMode = "none";
  focusOutline.visible = false;
  sheetContainer.addChild(focusOutline);

  const sheetDropOutline = new Graphics()
    .rect(
      viewGeometry.activeBounds.x,
      viewGeometry.activeBounds.y,
      viewGeometry.activeBounds.width,
      viewGeometry.activeBounds.height,
    )
    .stroke({
      ...SHEET_VISUAL_STYLE.technicalOutlineStroke,
      width: 2,
    });
  sheetDropOutline.label = `sheet-photo-drop-${sheet.sheetId}`;
  sheetDropOutline.eventMode = "none";
  sheetDropOutline.visible = false;
  sheetContainer.addChild(sheetDropOutline);

  return {
    container: sheetContainer,
    signature,
    photoNodes,
    placeholderLabels,
    inactiveSideGradient: inactiveSide?.gradient ?? null,
    frameSelections,
    frameDropOutlines,
    focusOutline,
    sheetDropOutline,
    sheetBar,
    viewBounds: viewGeometry.visibleOuterBounds,
    activeOffsetXPx: presentation.activeOffsetXPx,
  };
}

export function applyPlaceholderLabelScale(
  node: SheetRenderNode,
  canvasScale: number,
) {
  const inverseScale = 1 / Math.max(canvasScale, Number.EPSILON);
  for (const label of node.placeholderLabels) {
    label.scale.set(inverseScale);
  }
}

export function destroySheetRenderNode(node: SheetRenderNode) {
  stopSheetBarTransition(node.sheetBar);
  node.container.destroy({ children: true });
  node.inactiveSideGradient?.destroy();
}

export function setPhotoPanAids(
  node: PhotoRenderNode,
  visible: boolean,
) {
  node.outsideLayer.visible = visible;
  node.thirdsGuides.visible = visible;
}

export function applyPhotoZoomPreview(
  node: PhotoRenderNode,
  targetZoom: number,
) {
  const zoomed = node.geometry.zoom(targetZoom);
  applyPhotoPlacementPreview(node, zoomed.zoom, zoomed.placement);
}

export function applyPhotoPlacementPreview(
  node: PhotoRenderNode,
  targetZoom: number,
  placement: CanvasPhotoPlacement,
) {
  const factor = targetZoom / node.baseZoom;
  setPhotoLayersScale(node, node.baseScaleX * factor, factor);
  setPhotoPreviewPosition(node, placement.center.x, placement.center.y);
}

export function resetPhotoPreview(node: PhotoRenderNode) {
  setPhotoLayersScale(node, node.baseScaleX, 1);
  setPhotoPreviewPosition(node, node.originalX, node.originalY);
}

export function setPhotoPreviewPosition(
  node: PhotoRenderNode,
  x: number,
  y: number,
) {
  node.layer.position.set(x, y);
  node.outsideLayer.position.set(x, y);
}

function createPhotoPreviewLayer({
  label,
  drawWidth,
  drawHeight,
  center,
  rotationDegrees,
  mirrorX,
  palette,
  previewTexture,
}: PhotoPreviewLayerOptions) {
  const photoLayer = new Container();
  photoLayer.label = label;
  photoLayer.pivot.set(drawWidth / 2, drawHeight / 2);
  photoLayer.position.set(center.x, center.y);
  photoLayer.rotation = (rotationDegrees * Math.PI) / 180;
  photoLayer.scale.set(mirrorX ? -1 : 1, 1);

  if (previewTexture) {
    const sprite = new Sprite({
      texture: previewTexture,
      width: drawWidth,
      height: drawHeight,
    });
    photoLayer.addChild(sprite);
    return photoLayer;
  }

  const photoStyle = SHEET_VISUAL_STYLE.photo;
  for (let stripe = 0; stripe < photoStyle.stripeCount; stripe += 1) {
    const paletteIndex = photoPaletteIndexForStripe(stripe);
    photoLayer.addChild(
      new Graphics()
        .rect(
          (drawWidth / photoStyle.stripeCount) * stripe,
          0,
          drawWidth / photoStyle.stripeCount + photoStyle.stripeOverlapPx,
          drawHeight,
        )
        .fill({ color: pixiColor(palette[paletteIndex]) }),
    );
  }
  photoLayer.addChild(
    new Graphics()
      .circle(
        drawWidth * photoStyle.lightCenterXRatio,
        drawHeight * photoStyle.lightCenterYRatio,
        drawHeight * photoStyle.lightRadiusToHeightRatio,
      )
      .fill({
        color: pixiColor(photoStyle.lightColor),
        alpha: photoStyle.lightOpacity,
      }),
  );
  return photoLayer;
}

function createThirdsGuides(frameWidth: number, frameHeight: number) {
  const guides = new Graphics()
    .moveTo(frameWidth / 3, 0)
    .lineTo(frameWidth / 3, frameHeight)
    .moveTo((frameWidth * 2) / 3, 0)
    .lineTo((frameWidth * 2) / 3, frameHeight)
    .moveTo(0, frameHeight / 3)
    .lineTo(frameWidth, frameHeight / 3)
    .moveTo(0, (frameHeight * 2) / 3)
    .lineTo(frameWidth, (frameHeight * 2) / 3)
    .stroke({ color: 0xffffff, width: 1.2, alpha: 0.88 });
  guides.label = "photo-pan-thirds-guides";
  guides.eventMode = "none";
  guides.visible = false;
  return guides;
}

function setPhotoLayersScale(
  node: PhotoRenderNode,
  x: number,
  y: number,
) {
  node.layer.scale.set(x, y);
  node.outsideLayer.scale.set(x, y);
}
