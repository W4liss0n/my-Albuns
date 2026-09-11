import {
  AlphaFilter,
  Container,
  type FederatedPointerEvent,
  type FederatedWheelEvent,
  Graphics,
  Rectangle,
  Sprite,
  type FillGradient,
  type Texture,
} from "pixi.js";

import type {
  ComposedSheet,
  FrameResizeHandle,
  NormalizedPan,
  RectUm,
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
import { createDecorativeDropFeedback } from "./decorativeDropFeedback";
import { createPhotoBlackAndWhiteFilter } from "./photoBlackAndWhite";
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
  createDragPreview: () => { container: Container; bounds: Rectangle };
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
  framePlaceholders: ReturnType<typeof createCanvasFramePlaceholder>[];
  inactiveSideGradient: FillGradient | null;
  frameSelections: Map<string, FrameSelectionRenderNode>;
  frameSelectionLayer: Container;
  frameGroupSelection: { signature: string; node: FrameSelectionRenderNode } | null;
  frameDropOutlines: Map<string, Graphics>;
  frameContentDropHighlights: Map<string, Graphics>;
  decorativeDropFeedback: ReturnType<typeof createDecorativeDropFeedback> | null;
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
  onFrameTap: (sheetId: string, frameId: string, toggle: boolean) => void;
  onFrameContextMenu: (frameId: string, position: { x: number; y: number }) => void;
  onEmptyCanvasContextMenu: (sheetId: string, position: { x: number; y: number }) => void;
  onFrameGeometryStart: (
    frameId: string,
    handle: FrameResizeHandle | null,
    event: FederatedPointerEvent,
  ) => void;
  onPhotoPanStart: (
    photoNode: PhotoRenderNode,
    event: FederatedPointerEvent,
  ) => void;
  onPhotoContentDragStart: (frameId: string, event: FederatedPointerEvent) => void;
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
  blackAndWhite: boolean;
  palette: readonly string[];
  previewTexture?: Texture;
}

export function createSheetRenderNode(
  sheet: ComposedSheet,
  sheetBarMetadata: SheetBarMetadata | undefined,
  technicalGuides: CanvasTechnicalGuides | undefined,
  modePolicy: AlbumCanvasModePolicy,
  signature: string,
  callbacks: SheetRenderNodeCallbacks,
  decorativePreview?: import("../domain/project").DecorativeDropPreview | null,
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
    if (modePolicy.editingSheetId !== null || !(event.detail >= 2)) return false;
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
  sheetContainer.on("rightclick", (event: FederatedPointerEvent) => {
    if (modePolicy.editingSheetId === null || sheetBarMetadata?.layoutLocked || event.target !== sheetContainer) return;
    event.stopPropagation();
    callbacks.onEmptyCanvasContextMenu(sheet.sheetId, { x: event.clientX, y: event.clientY });
  });
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
      addDecorativeRenderNode(activeContent, sprite, background.clipRect);
    } else {
      const fallback = new Graphics()
        .rect(x, y, backgroundWidth, backgroundHeight)
        .fill({ color: pixiColor(SHEET_VISUAL_STYLE.mediaFallback.fill) });
      fallback.label = `background-media-fallback-${background.mediaId}`;
      fallback.eventMode = "none";
      addDecorativeRenderNode(activeContent, fallback, background.clipRect);
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
  const frameContentDropHighlights = new Map<string, Graphics>();
  const frameSelectionLayer = new Container();
  frameSelectionLayer.label = `frame-selection-layer-${sheet.sheetId}`;
  frameSelectionLayer.eventMode = "passive";
  const photoNodes: PhotoRenderNode[] = [];
  const framePlaceholders: ReturnType<typeof createCanvasFramePlaceholder>[] = [];
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
    frameContainer.cursor = modePolicy.showsFrameResizeHandles && !sheetBarMetadata?.layoutLocked
      ? "move" : "default";
    const frameContent = new Container();
    frameContent.label = `frame-content-${frame.frameId}`;
    frameContent.eventMode = "none";
    if (frame.opacityByte < 255) {
      const opacity = new AlphaFilter({ alpha: frame.opacityByte / 255 });
      frameContent.filters = [opacity];
      // AlphaFilter uses cached programs shared with other Frames.
      frameContent.on("destroyed", () => opacity.destroy());
    }

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
        blackAndWhite: frame.photo.blackAndWhite,
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
      const { viewport: photoViewport, layer: photoLayer, clip } = createClippedPhotoPreview({
        ...previewOptions,
        label: "photo-pan-inside-preview",
      }, frameWidth, frameHeight);
      const thirdsGuides = createThirdsGuides(frameWidth, frameHeight);
      frameContainer.addChild(
        outsidePhotoLayer,
        frameContent,
        thirdsGuides,
      );
      frameContent.addChild(photoViewport, clip);

      const baseZoom = frame.photo.placement.currentZoom;
      photoNode = {
        frameId: frame.frameId,
        createDragPreview: () => {
          const width = previewOptions.previewTexture?.orig.width ?? previewOptions.drawWidth;
          const height = previewOptions.previewTexture?.orig.height ?? previewOptions.drawHeight;
          // The drag represents the source Photo, independent of its Frame crop and transforms.
          return {
            container: createPhotoPreviewLayer({ ...previewOptions, label: "photo-drag-preview",
              drawWidth: width, drawHeight: height, center: { x: width / 2, y: height / 2 },
              rotationDegrees: 0, mirrorX: false, blackAndWhite: false }),
            bounds: new Rectangle(0, 0, width, height),
          };
        },
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
      frameContent.addChild(emptyPlaceholder.fill);
      frameContainer.addChild(frameContent, emptyPlaceholder.container);
      framePlaceholders.push(emptyPlaceholder);
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
    const frameBorder = frame.border;
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
    if (persistedBorder) frameContent.addChild(persistedBorder);
    frameContainer.addChild(outline);

    const frameSelection = createFrameSelectionRenderNode(
      frame.frameId,
      frameWidth,
      frameHeight,
      modePolicy.showsFrameResizeHandles &&
        sheetBarMetadata?.layoutLocked === false,
      (handle, event) => callbacks.onFrameGeometryStart(frame.frameId, handle, event),
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

    const contentDragStyle = SHEET_VISUAL_STYLE.frameContentDrag;
    const contentDropHighlight = new Graphics()
      .rect(frameX, frameY, frameWidth, frameHeight)
      .fill({ color: contentDragStyle.targetColor, alpha: contentDragStyle.targetFillOpacity })
      .stroke({ color: contentDragStyle.targetColor, width: contentDragStyle.targetOutlineWidthPx });
    contentDropHighlight.label = `frame-content-drop-${frame.frameId}`;
    contentDropHighlight.eventMode = "none";
    contentDropHighlight.visible = false;
    frameContentDropHighlights.set(frame.frameId, contentDropHighlight);
    frameSelectionLayer.addChild(contentDropHighlight);

    frameContainer.on("pointertap", (event: FederatedPointerEvent) => {
      if (event.button !== 0) return;
      event.stopPropagation();
      if (dispatchSheetDoubleTap(event)) return;
      if (!event.altKey || modePolicy.showsFrameResizeHandles) {
        callbacks.onFrameTap(sheet.sheetId, frame.frameId, modePolicy.showsFrameResizeHandles && event.ctrlKey);
      }
    });
    frameContainer.on("rightclick", (event: FederatedPointerEvent) => {
      if (!modePolicy.showsFrameResizeHandles) return;
      event.stopPropagation();
      callbacks.onFrameContextMenu(frame.frameId, { x: event.clientX, y: event.clientY });
    });
    frameContainer.on("pointerdown", (event: FederatedPointerEvent) => {
      if (modePolicy.showsFrameResizeHandles) {
        callbacks.onFrameGeometryStart(frame.frameId, null, event);
        return;
      }
      if (!modePolicy.enablesPhotoTransform || !photoNode) {
        return;
      }
      event.stopPropagation();
      if (event.altKey) callbacks.onPhotoPanStart(photoNode, event);
      else callbacks.onPhotoContentDragStart(frame.frameId, event);
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
      addDecorativeRenderNode(activeContent, overlay, composedOverlay.clipRect);
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
      addDecorativeRenderNode(activeContent, overlay, composedOverlay.clipRect);
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
  const decorativeDropFeedback = decorativePreview
    ? createDecorativeDropFeedback(decorativePreview, viewGeometry.activeBounds) : null;
  if (decorativeDropFeedback) sheetContainer.addChild(decorativeDropFeedback.container);

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
    framePlaceholders,
    inactiveSideGradient: inactiveSide?.gradient ?? null,
    frameSelections,
    frameSelectionLayer,
    frameGroupSelection: null,
    frameDropOutlines,
    frameContentDropHighlights,
    decorativeDropFeedback,
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
  for (const placeholder of node.framePlaceholders) {
    placeholder.applyCanvasScale(canvasScale);
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

function createClippedPhotoPreview(options: PhotoPreviewLayerOptions, width: number, height: number) {
  const viewport = new Container();
  const layer = createPhotoPreviewLayer(options);
  const clip = new Graphics().rect(0, 0, width, height).fill(0xffffff);
  viewport.addChild(layer);
  viewport.mask = clip;
  return { viewport, layer, clip };
}

function createPhotoPreviewLayer({
  label,
  drawWidth,
  drawHeight,
  center,
  rotationDegrees,
  mirrorX,
  blackAndWhite,
  palette,
  previewTexture,
}: PhotoPreviewLayerOptions) {
  const photoLayer = new Container();
  photoLayer.label = label;
  photoLayer.pivot.set(drawWidth / 2, drawHeight / 2);
  photoLayer.position.set(center.x, center.y);
  // Pixi scales in local coordinates: reverse the angle to mirror after rotation.
  photoLayer.rotation = ((mirrorX ? -rotationDegrees : rotationDegrees) * Math.PI) / 180;
  photoLayer.scale.set(mirrorX ? -1 : 1, 1);
  if (blackAndWhite) {
    const filter = createPhotoBlackAndWhiteFilter();
    photoLayer.filters = [filter];
    photoLayer.once("destroyed", () => filter.destroy(true));
  }

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

function addDecorativeRenderNode(
  parent: Container,
  node: Sprite | Graphics,
  clipRect?: RectUm,
) {
  parent.addChild(node);
  if (!clipRect) return;
  const mask = new Graphics().rect(
    clipRect.x * MICROMETER_TO_CANVAS_PIXEL,
    clipRect.y * MICROMETER_TO_CANVAS_PIXEL,
    clipRect.width * MICROMETER_TO_CANVAS_PIXEL,
    clipRect.height * MICROMETER_TO_CANVAS_PIXEL,
  ).fill({ color: 0xffffff });
  mask.label = `${node.label}-clip`;
  mask.eventMode = "none";
  parent.addChild(mask);
  node.mask = mask;
}
