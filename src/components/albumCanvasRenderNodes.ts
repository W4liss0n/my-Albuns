import {
  Container,
  type FederatedPointerEvent,
  type FederatedWheelEvent,
  Graphics,
  Rectangle,
  Sprite,
  Text,
  type Texture,
} from "pixi.js";

import type { ComposedSheet, NormalizedPan } from "../domain/project";
import {
  MICROMETER_TO_CANVAS_PIXEL,
  SHEET_LABEL_HEIGHT_PX,
} from "./canvasGeometry";
import {
  createPhotoGeometry,
  type CanvasPhotoPlacement,
  type CanvasPoint,
  type PhotoGeometry,
} from "./photoGeometry";
import {
  photoPaletteIndexForStripe,
  SHEET_VISUAL_STYLE,
} from "./sheetVisualStyle";

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
  selectionOutlines: Map<string, Graphics>;
  focusOutline: Graphics;
}

interface SheetRenderNodeCallbacks {
  previewTextureFor: (mediaId: string) => Texture | undefined;
  onSheetTap: (sheetId: string) => void;
  onFrameTap: (sheetId: string, frameId: string) => void;
  onPhotoPanStart: (
    frameContainer: Container,
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
  signature: string,
  callbacks: SheetRenderNodeCallbacks,
): SheetRenderNode {
  const sheetContainer = new Container();
  const width = sheet.widthUm * MICROMETER_TO_CANVAS_PIXEL;
  const height = sheet.heightUm * MICROMETER_TO_CANVAS_PIXEL;
  sheetContainer.eventMode = "static";
  sheetContainer.hitArea = new Rectangle(0, 0, width, height);
  sheetContainer.cursor = "default";
  sheetContainer.on("pointertap", (event: FederatedPointerEvent) => {
    if (event.target === sheetContainer) {
      callbacks.onSheetTap(sheet.sheetId);
    }
  });

  const shadow = new Graphics()
    .roundRect(8, 12, width, height, 4)
    .fill({ color: 0x121820, alpha: 0.2 });
  const surface = new Graphics()
    .roundRect(
      0,
      0,
      width,
      height,
      SHEET_VISUAL_STYLE.surface.cornerRadiusPx,
    )
    .fill({ color: hexToNumber(SHEET_VISUAL_STYLE.surface.fill) })
    .stroke({
      color: hexToNumber(SHEET_VISUAL_STYLE.surface.outline),
      width: SHEET_VISUAL_STYLE.surface.outlineWidthPx,
      alpha: SHEET_VISUAL_STYLE.surface.outlineOpacity,
    });
  sheetContainer.addChild(shadow, surface);

  const label = new Text({
    text: `LÂMINA ${String(sheet.number).padStart(2, "0")}`,
    style: {
      fontFamily: "Segoe UI",
      fontSize: 10,
      fontWeight: "600",
      fill: 0x77808a,
      letterSpacing: 1.4,
    },
  });
  label.position.set(2, -SHEET_LABEL_HEIGHT_PX);
  sheetContainer.addChild(label);

  if (sheet.activeSides === "both") {
    const centerLine = new Graphics()
      .moveTo(width / 2, 0)
      .lineTo(width / 2, height)
      .stroke({
        color: hexToNumber(SHEET_VISUAL_STYLE.centerLine.color),
        width: SHEET_VISUAL_STYLE.centerLine.widthPx,
        alpha: SHEET_VISUAL_STYLE.centerLine.opacity,
      });
    sheetContainer.addChild(centerLine);
  }

  const selectionOutlines = new Map<string, Graphics>();
  const photoNodes: PhotoRenderNode[] = [];
  for (const frame of sheet.frames) {
    const frameContainer = new Container();
    const frameX = frame.clipRect.x * MICROMETER_TO_CANVAS_PIXEL;
    const frameY = frame.clipRect.y * MICROMETER_TO_CANVAS_PIXEL;
    const frameWidth = frame.clipRect.width * MICROMETER_TO_CANVAS_PIXEL;
    const frameHeight = frame.clipRect.height * MICROMETER_TO_CANVAS_PIXEL;
    frameContainer.position.set(frameX, frameY);
    frameContainer.eventMode = "static";
    frameContainer.hitArea = new Rectangle(0, 0, frameWidth, frameHeight);
    frameContainer.cursor = frame.photo ? "grab" : "pointer";

    let photoNode: PhotoRenderNode | null = null;
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
      frameContainer.addChild(
        createPlaceholder(frameWidth, frameHeight),
        createPlaceholderCross(frameWidth, frameHeight),
      );
    }

    const outline = new Graphics()
      .rect(0, 0, frameWidth, frameHeight)
      .stroke({
        color: hexToNumber(SHEET_VISUAL_STYLE.frame.outline),
        width: SHEET_VISUAL_STYLE.frame.outlineWidthPx,
        alpha: SHEET_VISUAL_STYLE.frame.outlineOpacity,
      });
    const selectionOutline = new Graphics()
      .rect(0, 0, frameWidth, frameHeight)
      .stroke({ color: 0xb8874f, width: 3, alpha: 1 });
    selectionOutline.label = `frame-selection-${frame.frameId}`;
    selectionOutline.eventMode = "none";
    selectionOutline.visible = false;
    selectionOutlines.set(frame.frameId, selectionOutline);
    frameContainer.addChild(outline, selectionOutline);

    frameContainer.on("pointertap", (event: FederatedPointerEvent) => {
      event.stopPropagation();
      if (!event.altKey) {
        callbacks.onFrameTap(sheet.sheetId, frame.frameId);
      }
    });
    frameContainer.on("pointerdown", (event: FederatedPointerEvent) => {
      if (!event.altKey || !photoNode) return;
      event.stopPropagation();
      callbacks.onPhotoPanStart(frameContainer, photoNode, event);
    });
    frameContainer.on("wheel", (event: FederatedWheelEvent) => {
      if (!event.altKey || !photoNode) return;
      callbacks.onPhotoWheel(photoNode, event);
    });
    sheetContainer.addChild(frameContainer);
  }

  if (sheet.overlay) {
    const previewTexture = callbacks.previewTextureFor(sheet.overlay.mediaId);
    if (previewTexture) {
      const overlay = new Sprite({ texture: previewTexture });
      overlay.label = `decorative-overlay-${sheet.overlay.mediaId}`;
      overlay.position.set(
        sheet.overlay.drawRect.x * MICROMETER_TO_CANVAS_PIXEL,
        sheet.overlay.drawRect.y * MICROMETER_TO_CANVAS_PIXEL,
      );
      overlay.width =
        sheet.overlay.drawRect.width * MICROMETER_TO_CANVAS_PIXEL;
      overlay.height =
        sheet.overlay.drawRect.height * MICROMETER_TO_CANVAS_PIXEL;
      overlay.eventMode = "none";
      sheetContainer.addChild(overlay);
    } else {
      const overlayStyle = SHEET_VISUAL_STYLE.overlay;
      const overlay = new Graphics()
        .roundRect(
          overlayStyle.insetPx,
          overlayStyle.insetPx,
          width - overlayStyle.insetPx * 2,
          height - overlayStyle.insetPx * 2,
          overlayStyle.cornerRadiusPx,
        )
        .stroke({
          color: hexToNumber(overlayStyle.outline),
          width: overlayStyle.outlineWidthPx,
          alpha: overlayStyle.outlineOpacity,
        });
      overlay.label = `decorative-overlay-fallback-${sheet.overlay.mediaId}`;
      overlay.eventMode = "none";
      sheetContainer.addChild(overlay);
    }
  }

  if (sheet.activeSides !== "both") {
    const inactiveX = sheet.activeSides === "right" ? 0 : width / 2;
    const inactiveSide = new Graphics()
      .rect(inactiveX, 0, width / 2, height)
      .fill({
        color: hexToNumber(SHEET_VISUAL_STYLE.inactiveSide.fill),
        alpha: SHEET_VISUAL_STYLE.inactiveSide.opacity,
      });
    inactiveSide.label = `inactive-${
      sheet.activeSides === "right" ? "left" : "right"
    }-side`;
    inactiveSide.eventMode = "static";
    inactiveSide.cursor = "default";
    sheetContainer.addChild(inactiveSide);
  }

  const focusOutline = new Graphics()
    .roundRect(-5, -5, width + 10, height + 10, 7)
    .stroke({ color: 0xc99a5d, width: 2, alpha: 0.9 });
  focusOutline.label = `sheet-focus-${sheet.sheetId}`;
  focusOutline.eventMode = "none";
  focusOutline.visible = false;
  sheetContainer.addChild(focusOutline);

  return {
    container: sheetContainer,
    signature,
    photoNodes,
    selectionOutlines,
    focusOutline,
  };
}

export function destroySheetRenderNode(node: SheetRenderNode) {
  node.container.destroy({ children: true });
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
        .fill({ color: hexToNumber(palette[paletteIndex]) }),
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
        color: hexToNumber(photoStyle.lightColor),
        alpha: photoStyle.lightOpacity,
      }),
  );
  return photoLayer;
}

function createPlaceholder(frameWidth: number, frameHeight: number) {
  return new Graphics()
    .rect(0, 0, frameWidth, frameHeight)
    .fill({ color: hexToNumber(SHEET_VISUAL_STYLE.placeholder.fill) })
    .stroke({
      color: hexToNumber(SHEET_VISUAL_STYLE.placeholder.outline),
      width: SHEET_VISUAL_STYLE.placeholder.outlineWidthPx,
      alpha: SHEET_VISUAL_STYLE.placeholder.outlineOpacity,
    });
}

function createPlaceholderCross(
  frameWidth: number,
  frameHeight: number,
) {
  const style = SHEET_VISUAL_STYLE.placeholder;
  return new Graphics()
    .moveTo(frameWidth / 2 - style.crossHalfLengthPx, frameHeight / 2)
    .lineTo(frameWidth / 2 + style.crossHalfLengthPx, frameHeight / 2)
    .moveTo(frameWidth / 2, frameHeight / 2 - style.crossHalfLengthPx)
    .lineTo(frameWidth / 2, frameHeight / 2 + style.crossHalfLengthPx)
    .stroke({
      color: hexToNumber(style.crossColor),
      width: style.crossWidthPx,
      alpha: style.crossOpacity,
    });
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

function hexToNumber(value: string): number {
  return Number.parseInt(value.replace("#", ""), 16);
}
