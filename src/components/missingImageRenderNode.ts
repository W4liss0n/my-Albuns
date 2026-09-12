import { Container, Graphics, Text } from "pixi.js";

import missingImageSymbol from "../ui/missingImageSymbol.svg?raw";

/** Editor-only representation: it belongs to the Frame, not the transformed Photo. */
export function createMissingImageRenderNode(frameId: string, width: number, height: number) {
  const fill = new Graphics().rect(0, 0, width, height).fill({ color: 0xefede8 });
  fill.label = `frame-missing-image-fill-${frameId}`;
  fill.eventMode = "none";
  const container = new Container();
  container.label = `frame-missing-image-${frameId}`;
  container.eventMode = "none";
  container.position.set(width / 2, height / 2);
  const symbol = new Graphics().svg(missingImageSymbol);
  symbol.label = `frame-missing-image-symbol-${frameId}`;
  symbol.position.set(-24, -36);
  symbol.scale.set(2);
  const label = new Text({ text: "Imagem ausente", style: {
    fontFamily: '"Helvetica Neue", "Segoe UI Variable", "Segoe UI", system-ui, sans-serif',
    fontSize: 12, fill: 0x7d766a,
  } });
  label.label = `frame-missing-image-label-${frameId}`;
  label.anchor.set(0.5, 0);
  label.position.set(0, 22);
  container.addChild(symbol, label);
  const contentWidth = Math.max(48, label.width);
  const contentHeight = 58 + label.height;
  return { container, fill, applyCanvasScale(canvasScale: number) {
    container.scale.set(Math.min(1 / Math.max(canvasScale, Number.EPSILON),
      width / (contentWidth + 24), height / (contentHeight + 24)));
  } };
}
