import { Container, FillGradient, Graphics, Text } from "pixi.js";

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
import {
  activePageHorizontalEdges,
  type CanvasBounds,
} from "./canvasSheetViewGeometry";
import { pixiColor } from "./pixiColor";
import {
  inactiveSideGradientOrientation,
  SHEET_VISUAL_STYLE,
} from "./sheetVisualStyle";

export function createSheetSurfaceRenderNodes(
  sheet: ComposedSheet,
  bounds: CanvasBounds,
) {
  const shadowStyle = SHEET_VISUAL_STYLE.canvasShadow;
  const depthShadow = createLayeredSheetShadow(
    `sheet-shadow-depth-${sheet.sheetId}`,
    bounds,
    shadowStyle.depth,
  );
  const closeShadow = createLayeredSheetShadow(
    `sheet-shadow-close-${sheet.sheetId}`,
    bounds,
    shadowStyle.close,
  );

  const surface = new Graphics()
    .rect(bounds.x, bounds.y, bounds.width, bounds.height)
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
  bounds: CanvasBounds,
  style: LayeredShadowStyle,
) {
  const shadow = new Graphics();
  for (let step = style.steps; step >= 1; step -= 1) {
    const spread = (style.spreadPx * step) / style.steps;
    shadow
      .rect(
        bounds.x - spread,
        bounds.y + style.offsetYPx - spread,
        bounds.width + spread * 2,
        bounds.height + spread * 2,
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
  presentation: CanvasSheetPresentation,
  bounds: CanvasBounds,
) {
  const centerX = presentation.visualWidthPx / 2;
  const centerLine = new Graphics()
    .moveTo(centerX, bounds.y)
    .lineTo(centerX, bounds.y + bounds.height)
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
  bounds: CanvasBounds | null,
) {
  if (sheet.activeSides === "both" || bounds === null) {
    return null;
  }
  const style = SHEET_VISUAL_STYLE.inactiveSide;
  const inactiveSide = new Container();
  inactiveSide.label = `sheet-inactive-side-${sheet.sheetId}`;
  inactiveSide.eventMode = "none";
  inactiveSide.position.set(bounds.x, bounds.y);

  const orientation = inactiveSideGradientOrientation(sheet.activeSides);
  const gradient = new FillGradient({
    type: "linear",
    start: { x: orientation.startX, y: 0 },
    end: { x: orientation.endX, y: 0 },
    colorStops: [
      { offset: 0, color: style.outerEdge },
      { offset: style.bodyStopOffset, color: style.body },
      { offset: 1, color: style.fold },
    ],
  });
  const gradientSurface = new Graphics()
    .rect(0, 0, bounds.width, bounds.height)
    .fill(gradient);
  gradientSurface.label = `sheet-inactive-side-gradient-${sheet.sheetId}`;
  gradientSurface.eventMode = "none";
  inactiveSide.addChild(gradientSurface);
  return { container: inactiveSide, gradient };
}

export function createCanvasFramePlaceholder(
  frameId: string,
  frameWidth: number,
  frameHeight: number,
) {
  const style = SHEET_VISUAL_STYLE.framePlaceholder;
  const placeholder = new Container();
  placeholder.label = `frame-placeholder-${frameId}`;
  placeholder.eventMode = "none";
  const base = new Graphics()
    .rect(0, 0, frameWidth, frameHeight)
    .fill({ color: pixiColor(style.fill) });
  base.label = `frame-placeholder-base-${frameId}`;
  base.eventMode = "none";

  const label = new Text({
    text: "Adicionar Foto",
    style: {
      fontFamily:
        '"Helvetica Neue", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif',
      fontSize: style.labelFontSizePx,
      fontWeight: "500",
      fill: pixiColor(style.labelText),
    },
  });
  label.label = `frame-placeholder-label-${frameId}`;
  label.anchor.set(0.5);
  label.position.set(frameWidth / 2, frameHeight / 2);
  label.eventMode = "none";
  placeholder.addChild(base, label);
  return { container: placeholder, label };
}

export function createSheetTechnicalGuideNodes(
  sheet: ComposedSheet,
  guides: CanvasTechnicalGuides | undefined,
) {
  if (!guides) return [];
  const geometry = createCanvasGuideGeometry(sheet, guides);
  const edges = activePageHorizontalEdges(sheet.activeSides);
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
  activeBounds: CanvasBounds,
) {
  const mask = new Graphics()
    .rect(
      activeBounds.x,
      activeBounds.y,
      activeBounds.width,
      activeBounds.height,
    )
    .fill(0xffffff);
  mask.label = `sheet-bleed-mask-${sheet.sheetId}`;
  mask.eventMode = "none";
  return mask;
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
  edges: ReturnType<typeof activePageHorizontalEdges>;
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
