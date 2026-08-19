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
import { pixiColor } from "./pixiColor";
import {
  createPhotoGeometry,
  type CanvasPhotoPlacement,
  type CanvasPoint,
  type PhotoGeometry,
} from "./photoGeometry";
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
  selectionOutlines: Map<string, Graphics>;
  focusOutline: Graphics;
  sheetBar: SheetBarRenderNode;
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
  const width = presentation.visualWidthPx;
  const height = sheet.heightUm * MICROMETER_TO_CANVAS_PIXEL;
  sheetContainer.label = `canvas-sheet-${sheet.sheetId}`;
  sheetContainer.eventMode = "static";
  sheetContainer.hitArea = new Rectangle(0, 0, width, height);
  sheetContainer.cursor = "default";
  sheetContainer.on("pointertap", (event: FederatedPointerEvent) => {
    if (event.target === sheetContainer) {
      if (event.detail >= 2) {
        callbacks.onSheetDoubleTap(sheet.sheetId);
      } else {
        callbacks.onSheetTap(sheet.sheetId);
      }
    }
  });

  sheetContainer.addChild(
    ...createSheetSurfaceRenderNodes(sheet, width, height),
  );
  const inactiveSide = createSheetInactiveSide(
    sheet,
    presentation,
    height,
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
        .fill({ color: 0xd8dee2 });
      fallback.label = `background-media-fallback-${background.mediaId}`;
      fallback.eventMode = "none";
      activeContent.addChild(fallback);
    }
  }

  const centerLine = createSheetCenterLine(sheet, width, height);
  if (centerLine) sheetContainer.addChild(centerLine);

  const selectionOutlines = new Map<string, Graphics>();
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
    const selectionOutline = new Graphics()
      .rect(0, 0, frameWidth, frameHeight)
      .stroke({ color: 0x2f7fba, width: 3, alpha: 1 });
    selectionOutline.label = `frame-selection-${frame.frameId}`;
    selectionOutline.eventMode = "none";
    selectionOutline.visible = false;
    selectionOutlines.set(frame.frameId, selectionOutline);
    frameContainer.addChild(
      outline,
      ...(persistedBorder ? [persistedBorder] : []),
      selectionOutline,
    );

    frameContainer.on("pointertap", (event: FederatedPointerEvent) => {
      event.stopPropagation();
      if (event.detail >= 2) {
        callbacks.onSheetDoubleTap(sheet.sheetId);
        return;
      }
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

  if (modePolicy.masksBleed) {
    const bleedMask = createSheetBleedMask(sheet, technicalGuides);
    if (bleedMask) activeContent.addChild(bleedMask);
  }
  if (modePolicy.showsTechnicalGuides) {
    for (const guide of createSheetTechnicalGuideNodes(
      sheet,
      technicalGuides,
    )) {
      activeContent.addChild(guide);
    }
  }

  const sheetBar = createSheetBarRenderNode(
    sheet,
    sheetBarMetadata,
    presentation,
  );
  sheetBar.container.visible = modePolicy.showsSheetBar;
  sheetBar.container.on(
    "pointertap",
    (event: FederatedPointerEvent) => {
      if (event.target === sheetBar.container) {
        if (event.detail >= 2) {
          callbacks.onSheetDoubleTap(sheet.sheetId);
        } else {
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
    .rect(0, 0, width, height)
    .stroke({
      alignment: 0,
      color: 0x2f7fba,
      width: 2,
      alpha: 0.9,
    });
  focusOutline.label = `sheet-focus-${sheet.sheetId}`;
  focusOutline.eventMode = "none";
  focusOutline.visible = false;
  sheetContainer.addChild(focusOutline);

  return {
    container: sheetContainer,
    signature,
    photoNodes,
    placeholderLabels,
    inactiveSideGradient: inactiveSide?.gradient ?? null,
    selectionOutlines,
    focusOutline,
    sheetBar,
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
