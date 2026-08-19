import { Container, Graphics, Text } from "pixi.js";

import type { ComposedSheet } from "../domain/project";
import type { SheetBarMetadata } from "./albumCanvasContract";
import { pixiColor } from "./pixiColor";
import { SHEET_VISUAL_STYLE } from "./sheetVisualStyle";

export interface SheetBarRenderNode {
  container: Container;
  horizontallyFixedElements: Container[];
  sheetNumber: Text;
  swapPlaceholder: Text;
  transitionTimer: ReturnType<typeof setTimeout> | null;
  width: number;
}

export function createSheetBarRenderNode(
  sheet: ComposedSheet,
  metadata: SheetBarMetadata | undefined,
  width: number,
): SheetBarRenderNode {
  const style = SHEET_VISUAL_STYLE.sheetBar;
  const bar = new Container();
  bar.label = `sheet-bar-${sheet.sheetId}`;
  bar.alpha = 0;
  bar.eventMode = "none";

  const surface = new Graphics()
    .rect(0, 0, width, style.heightPx)
    .fill({
      color: pixiColor(style.surface),
      alpha: style.surfaceOpacity,
    });
  surface.label = `sheet-bar-surface-${sheet.sheetId}`;
  surface.eventMode = "none";
  const separator = new Graphics()
    .moveTo(0, style.heightPx - 0.5)
    .lineTo(width, style.heightPx - 0.5)
    .stroke({
      color: pixiColor(style.separator),
      width: 1,
      alpha: style.separatorOpacity,
    });
  separator.label = `sheet-bar-separator-${sheet.sheetId}`;
  separator.eventMode = "none";
  bar.addChild(surface, separator);

  const pageNumbers = metadata?.pageNumbers ?? [];
  const horizontallyFixedElements: Container[] = [];
  for (const page of sheetBarPages(sheet, pageNumbers, width)) {
    const pageLabel = createSheetBarText({
      text: String(page.number),
      x: page.x,
      y: style.heightPx / 2,
      label: `sheet-bar-page-${page.side}-${sheet.sheetId}`,
      fill: style.text,
      fontSize: style.pageFontSizePx,
    });
    horizontallyFixedElements.push(pageLabel);
    bar.addChild(pageLabel);
  }

  // Fidelity placeholders: these controls intentionally have no command
  // until the Sheet Bar interaction flow is implemented.
  const swapPlaceholder = createSheetBarText({
    text: "⇄",
    x: 22,
    y: style.heightPx / 2,
    label: `placeholder-sheet-bar-swap-${sheet.sheetId}`,
    fill: style.action,
    fontSize: 15,
  });
  const layoutPlaceholder = createLayoutPlaceholder(
    sheet.sheetId,
    width / 2,
    style.heightPx / 2,
  );
  swapPlaceholder.alpha = style.placeholderActionOpacity;
  layoutPlaceholder.alpha = style.placeholderActionOpacity;
  const sheetNumber = createSheetBarText({
    text: `L${String(sheet.number).padStart(2, "0")}`,
    x: width - 11,
    y: style.heightPx / 2,
    label: `sheet-bar-number-${sheet.sheetId}`,
    fill: style.action,
    fontSize: style.numberFontSizePx,
    anchorX: 1,
    fontWeight: "500",
    letterSpacing: 1,
  });
  bar.addChild(swapPlaceholder, layoutPlaceholder, sheetNumber);
  horizontallyFixedElements.push(
    swapPlaceholder,
    layoutPlaceholder,
    sheetNumber,
  );
  return {
    container: bar,
    horizontallyFixedElements,
    sheetNumber,
    swapPlaceholder,
    transitionTimer: null,
    width,
  };
}

export function setSheetBarHovered(
  node: SheetBarRenderNode,
  hovered: boolean,
) {
  stopSheetBarTransition(node);
  const style = SHEET_VISUAL_STYLE.sheetBar;
  const initialOpacity = node.container.alpha;
  const targetOpacity = hovered ? style.hoverOpacity : 0;
  let elapsedMs = 0;
  const tick = () => {
    elapsedMs += style.hoverTransitionFrameMs;
    const progress = Math.min(
      1,
      elapsedMs / style.hoverTransitionDurationMs,
    );
    const easedProgress = progress * progress * (3 - 2 * progress);
    node.container.alpha =
      initialOpacity + (targetOpacity - initialOpacity) * easedProgress;
    if (progress < 1) {
      node.transitionTimer = setTimeout(
        tick,
        style.hoverTransitionFrameMs,
      );
    } else {
      node.transitionTimer = null;
    }
  };
  node.transitionTimer = setTimeout(tick, style.hoverTransitionFrameMs);
}

export function stopSheetBarTransition(node: SheetBarRenderNode) {
  if (node.transitionTimer === null) return;
  clearTimeout(node.transitionTimer);
  node.transitionTimer = null;
}

export function applySheetBarScale(
  node: SheetBarRenderNode,
  canvasScale: number,
) {
  const safeScale = Math.max(canvasScale, Number.EPSILON);
  node.container.scale.y = 1 / safeScale;
  for (const element of node.horizontallyFixedElements) {
    element.scale.x = 1 / safeScale;
  }
  node.swapPlaceholder.position.x = 22 / safeScale;
  node.sheetNumber.position.x = node.width - 11 / safeScale;
}

function sheetBarPages(
  sheet: ComposedSheet,
  pageNumbers: readonly number[],
  width: number,
) {
  if (sheet.activeSides === "both") {
    return [
      { number: pageNumbers[0], side: "left", x: width / 4 },
      { number: pageNumbers[1], side: "right", x: (width * 3) / 4 },
    ].filter(
      (page): page is { number: number; side: string; x: number } =>
        page.number !== undefined,
    );
  }
  const pageNumber = pageNumbers[0];
  return pageNumber === undefined
    ? []
    : [{ number: pageNumber, side: sheet.activeSides, x: width / 2 }];
}

function createSheetBarText({
  text,
  x,
  y,
  label,
  fill,
  fontSize,
  anchorX = 0.5,
  fontWeight = "400",
  letterSpacing = 0,
}: {
  text: string;
  x: number;
  y: number;
  label: string;
  fill: string;
  fontSize: number;
  anchorX?: number;
  fontWeight?: "400" | "500";
  letterSpacing?: number;
}) {
  const textNode = new Text({
    text,
    style: {
      fontFamily: 'ui-monospace, "SFMono-Regular", Consolas, monospace',
      fontSize,
      fontWeight,
      fill: pixiColor(fill),
      letterSpacing,
    },
  });
  textNode.label = label;
  textNode.anchor.set(anchorX, 0.5);
  textNode.position.set(x, y);
  textNode.eventMode = "none";
  return textNode;
}

function createLayoutPlaceholder(
  sheetId: string,
  centerX: number,
  centerY: number,
) {
  const color = pixiColor(SHEET_VISUAL_STYLE.sheetBar.action);
  const icon = new Graphics();
  const size = 4.5;
  const gap = 2.5;
  const left = -size - gap / 2;
  const top = -size - gap / 2;
  for (let row = 0; row < 2; row += 1) {
    for (let column = 0; column < 2; column += 1) {
      icon.rect(
        left + column * (size + gap),
        top + row * (size + gap),
        size,
        size,
      );
    }
  }
  icon.stroke({ color, width: 1.15, alpha: 1 });
  icon.label = `placeholder-sheet-bar-layout-${sheetId}`;
  icon.eventMode = "none";
  icon.position.set(centerX, centerY);
  return icon;
}
