import { Container, Graphics } from "pixi.js";

import type { ComposedSheet } from "../domain/project";
import {
  createSheetGuideGeometry,
  SHEET_GUIDE_STYLE,
} from "../ui/sheetGuideGeometry";
import type { CanvasTechnicalGuides } from "./albumCanvasContract";
import {
  type CanvasSheetPresentation,
  MICROMETER_TO_CANVAS_PIXEL,
} from "./canvasGeometry";
import { pixiColor } from "./pixiColor";
import { SHEET_VISUAL_STYLE } from "./sheetVisualStyle";

export function createSheetSurfaceRenderNodes(
  sheet: ComposedSheet,
  width: number,
  height: number,
) {
  const shadowStyle = SHEET_VISUAL_STYLE.canvasShadow;
  const depthShadow = createLayeredSheetShadow(
    `sheet-shadow-depth-${sheet.sheetId}`,
    width,
    height,
    shadowStyle.depth,
  );
  const closeShadow = createLayeredSheetShadow(
    `sheet-shadow-close-${sheet.sheetId}`,
    width,
    height,
    shadowStyle.close,
  );

  const surface = new Graphics()
    .rect(0, 0, width, height)
    .fill({ color: pixiColor(sheet.base.rgb) })
    .stroke({
      color: pixiColor(SHEET_VISUAL_STYLE.surface.outline),
      width: SHEET_VISUAL_STYLE.surface.outlineWidthPx,
      alpha: SHEET_VISUAL_STYLE.surface.outlineOpacity,
      pixelLine: true,
    });
  surface.label = `sheet-surface-${sheet.sheetId}`;
  surface.eventMode = "none";
  return [depthShadow, closeShadow, surface];
}

interface LayeredShadowStyle {
  color: string;
  offsetYPx: number;
  opacity: number;
  spreadPx: number;
  steps: number;
}

function createLayeredSheetShadow(
  label: string,
  width: number,
  height: number,
  style: LayeredShadowStyle,
) {
  const shadow = new Graphics();
  for (let step = style.steps; step >= 1; step -= 1) {
    const spread = (style.spreadPx * step) / style.steps;
    shadow
      .rect(
        -spread,
        style.offsetYPx - spread,
        width + spread * 2,
        height + spread * 2,
      )
      .fill({
        color: pixiColor(style.color),
        alpha: style.opacity / style.steps,
      });
  }
  shadow.label = label;
  shadow.eventMode = "none";
  return shadow;
}

export function createSheetCenterLine(
  sheet: ComposedSheet,
  width: number,
  height: number,
) {
  const centerLine = new Graphics()
    .moveTo(width / 2, 0)
    .lineTo(width / 2, height)
    .stroke({
      color: pixiColor(SHEET_VISUAL_STYLE.centerLine.color),
      width: SHEET_VISUAL_STYLE.centerLine.widthPx,
      alpha: SHEET_VISUAL_STYLE.centerLine.opacity,
      pixelLine: true,
    });
  centerLine.label = `sheet-center-line-${sheet.sheetId}`;
  centerLine.eventMode = "none";
  return centerLine;
}

export function createSheetInactiveSide(
  sheet: ComposedSheet,
  presentation: CanvasSheetPresentation,
  height: number,
) {
  if (presentation.inactiveOffsetXPx === null) return null;
  const style = SHEET_VISUAL_STYLE.inactiveSide;
  const inactiveSide = new Container();
  inactiveSide.label = `sheet-inactive-side-${sheet.sheetId}`;
  inactiveSide.eventMode = "none";
  inactiveSide.position.set(presentation.inactiveOffsetXPx, 0);

  const base = new Graphics()
    .rect(0, 0, presentation.activeWidthPx, height)
    .fill({ color: pixiColor(style.fill) });
  base.label = `sheet-inactive-side-base-${sheet.sheetId}`;
  base.eventMode = "none";

  const foldShadow = new Graphics();
  for (let step = style.foldShadowSteps; step >= 1; step -= 1) {
    const width =
      (style.foldShadowWidthPx * step) / style.foldShadowSteps;
    const x = sheet.activeSides === "right"
      ? presentation.activeWidthPx - width
      : 0;
    foldShadow.rect(x, 0, width, height).fill({
      color: pixiColor(style.foldShadow),
      alpha: style.foldShadowOpacity / style.foldShadowSteps,
    });
  }
  foldShadow.label = `sheet-inactive-side-fold-shadow-${sheet.sheetId}`;
  foldShadow.eventMode = "none";
  inactiveSide.addChild(base, foldShadow);
  return inactiveSide;
}

export function createCanvasFramePlaceholder(
  frameId: string,
  frameWidth: number,
  frameHeight: number,
) {
  const style = SHEET_VISUAL_STYLE.canvasPlaceholder;
  const placeholder = new Container();
  placeholder.label = `frame-placeholder-${frameId}`;
  placeholder.eventMode = "none";
  const base = new Graphics()
    .rect(0, 0, frameWidth, frameHeight)
    .fill({ color: pixiColor(style.light) });
  base.label = `frame-placeholder-base-${frameId}`;
  base.eventMode = "none";

  const stripes = new Graphics();
  const period = style.stripeWidthPx + style.stripeGapPx;
  for (
    let offset = -frameHeight;
    offset < frameWidth;
    offset += period
  ) {
    stripes
      .moveTo(offset, 0)
      .lineTo(offset + style.stripeWidthPx, 0)
      .lineTo(offset + style.stripeWidthPx + frameHeight, frameHeight)
      .lineTo(offset + frameHeight, frameHeight)
      .lineTo(offset, 0);
  }
  stripes.fill({ color: pixiColor(style.dark) });
  stripes.label = `frame-placeholder-stripes-${frameId}`;
  stripes.eventMode = "none";
  const clip = new Graphics()
    .rect(0, 0, frameWidth, frameHeight)
    .fill(0xffffff);
  clip.label = `frame-placeholder-mask-${frameId}`;
  clip.eventMode = "none";
  stripes.mask = clip;
  placeholder.addChild(base, stripes, clip);
  return placeholder;
}

export function createSheetTechnicalGuideNodes(
  sheet: ComposedSheet,
  guides: CanvasTechnicalGuides | undefined,
) {
  if (!guides) return [];
  const geometry = createCanvasGuideGeometry(sheet, guides);
  const edges = activeGuideEdges(sheet.activeSides);
  return [
    guides.bleedUm > 0
      ? createDashedGuide({
          color: SHEET_GUIDE_STYLE.bleed,
          edges,
          geometry,
          inset: geometry.bleedInset,
          label: `sheet-bleed-guide-${sheet.sheetId}`,
        })
      : null,
    guides.safetyUm > 0
      ? createDashedGuide({
          color: SHEET_GUIDE_STYLE.safety,
          edges,
          geometry,
          inset: geometry.safetyInset,
          label: `sheet-safety-guide-${sheet.sheetId}`,
        })
      : null,
  ].filter((guide): guide is Graphics => guide !== null);
}

export function createSheetBleedMask(
  sheet: ComposedSheet,
  guides: CanvasTechnicalGuides | undefined,
) {
  if (!guides || guides.bleedUm <= 0) return null;
  const geometry = createCanvasGuideGeometry(sheet, guides);
  const inset = geometry.bleedInset;
  if (inset <= 0) return null;

  const { width, height } = geometry;
  const innerHeight = Math.max(0, height - inset * 2);
  const edges = activeGuideEdges(sheet.activeSides);
  const mask = new Graphics().rect(0, 0, width, inset);
  if (edges.right) {
    mask.rect(width - inset, inset, inset, innerHeight);
  }
  mask.rect(0, height - inset, width, inset);
  if (edges.left) {
    mask.rect(0, inset, inset, innerHeight);
  }
  mask.fill({
    color: pixiColor(SHEET_VISUAL_STYLE.bleedMask.fill),
    alpha: SHEET_VISUAL_STYLE.bleedMask.opacity,
  });
  mask.label = `sheet-bleed-mask-${sheet.sheetId}`;
  mask.eventMode = "none";
  return mask;
}

interface GuideEdges {
  right: boolean;
  left: boolean;
}

function activeGuideEdges(
  activeSides: ComposedSheet["activeSides"],
): GuideEdges {
  return {
    right: activeSides !== "left",
    left: activeSides !== "right",
  };
}

function createCanvasGuideGeometry(
  sheet: ComposedSheet,
  guides: CanvasTechnicalGuides,
) {
  const geometry = createSheetGuideGeometry({
    bleedUm: guides.bleedUm,
    heightUm: sheet.heightUm,
    safetyUm: guides.safetyUm,
  });
  return {
    bleedInset: geometry.bleedInsetUm * MICROMETER_TO_CANVAS_PIXEL,
    dashGap: geometry.dashGapUm * MICROMETER_TO_CANVAS_PIXEL,
    dashLength: geometry.dashLengthUm * MICROMETER_TO_CANVAS_PIXEL,
    height: sheet.heightUm * MICROMETER_TO_CANVAS_PIXEL,
    safetyInset: geometry.safetyInsetUm * MICROMETER_TO_CANVAS_PIXEL,
    strokeWidth: geometry.strokeWidthUm * MICROMETER_TO_CANVAS_PIXEL,
    width: sheet.widthUm * MICROMETER_TO_CANVAS_PIXEL,
  };
}

type CanvasGuideGeometry = ReturnType<typeof createCanvasGuideGeometry>;

interface DashedGuideOptions {
  color: string;
  edges: GuideEdges;
  geometry: CanvasGuideGeometry;
  inset: number;
  label: string;
}

function createDashedGuide({
  color,
  edges,
  geometry: {
    dashGap,
    dashLength,
    height: surfaceHeight,
    strokeWidth,
    width: surfaceWidth,
  },
  inset,
  label,
}: DashedGuideOptions) {
  const guide = new Graphics();
  const left = edges.left ? inset : 0;
  const right = edges.right
    ? Math.max(inset, surfaceWidth - inset)
    : surfaceWidth;
  const bottom = Math.max(inset, surfaceHeight - inset);
  drawDashedLine(guide, left, inset, right, inset, dashLength, dashGap);
  if (edges.right) {
    drawDashedLine(guide, right, inset, right, bottom, dashLength, dashGap);
  }
  drawDashedLine(guide, right, bottom, left, bottom, dashLength, dashGap);
  if (edges.left) {
    drawDashedLine(guide, left, bottom, left, inset, dashLength, dashGap);
  }
  guide.stroke({
    color: pixiColor(color),
    width: strokeWidth,
    alpha: SHEET_GUIDE_STYLE.opacity,
  });
  guide.label = label;
  guide.eventMode = "none";
  return guide;
}

function drawDashedLine(
  graphics: Graphics,
  startX: number,
  startY: number,
  endX: number,
  endY: number,
  dashLength: number,
  dashGap: number,
) {
  const deltaX = endX - startX;
  const deltaY = endY - startY;
  const length = Math.hypot(deltaX, deltaY);
  if (length <= 0) return;
  const step = dashLength + dashGap;
  for (let distance = 0; distance < length; distance += step) {
    const dashEnd = Math.min(length, distance + dashLength);
    graphics
      .moveTo(
        startX + (deltaX * distance) / length,
        startY + (deltaY * distance) / length,
      )
      .lineTo(
        startX + (deltaX * dashEnd) / length,
        startY + (deltaY * dashEnd) / length,
      );
  }
}
