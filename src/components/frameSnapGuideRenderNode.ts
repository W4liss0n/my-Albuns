import { Container, Graphics, Text } from "pixi.js";
import type { DisplayUnit, FrameSnapGuide } from "../domain/project";
import { formatPhysicalMeasurement } from "../application/physicalMeasurements";
import { MICROMETER_TO_CANVAS_PIXEL } from "./canvasGeometry";

const SNAP_COLOR = 0xd52b91;

export function createFrameSnapGuideRenderNode(widthUm: number, heightUm: number) {
  const container = new Container();
  container.label = "frame-snap-guides";
  container.eventMode = "none";
  const content = new Container();
  const clip = new Graphics().rect(0, 0, widthUm * MICROMETER_TO_CANVAS_PIXEL, heightUm * MICROMETER_TO_CANVAS_PIXEL).fill(0xffffff);
  content.mask = clip;
  container.addChild(content, clip);
  let signature = "";
  return {
    container,
    update(guides: readonly FrameSnapGuide[], scale: number, unit: DisplayUnit) {
      const nextSignature = JSON.stringify([guides, scale, unit]);
      if (signature === nextSignature) return;
      signature = nextSignature;
      for (const child of content.removeChildren()) child.destroy({ children: true });
      container.visible = guides.length > 0;
      const lines = consolidateGuides(guides);
      const labels: Array<{ x: number; y: number }> = [];
      for (const guide of lines) {
        const [x1, y1, x2, y2] = [guide.x1, guide.y1, guide.x2, guide.y2].map((value) => value * MICROMETER_TO_CANVAS_PIXEL);
        const drawing = new Graphics().moveTo(x1, y1).lineTo(x2, y2);
        drawing.label = `frame-snap-${guide.kind}-${guide.axis}`;
        drawing.eventMode = "none";
        if (guide.measurementUm !== null) {
          const tick = 3 / scale;
          if (guide.axis === "x") {
            drawing.moveTo(x1, y1 - tick).lineTo(x1, y1 + tick).moveTo(x2, y2 - tick).lineTo(x2, y2 + tick);
          } else {
            drawing.moveTo(x1 - tick, y1).lineTo(x1 + tick, y1).moveTo(x2 - tick, y2).lineTo(x2 + tick, y2);
          }
          const text = new Text({
            text: formatPhysicalMeasurement(Math.round(guide.measurementUm), unit),
            style: { fontFamily: '"Segoe UI", sans-serif', fontSize: 11, fill: SNAP_COLOR },
          });
          text.label = "frame-snap-measurement";
          text.eventMode = "none";
          text.anchor.set(0.5, 1);
          text.scale.set(1 / scale);
          const halfWidth = text.width / 2;
          const desiredX = (x1 + x2) / 2 - (guide.axis === "y" ? halfWidth + 5 / scale : 0);
          const x = Math.max(halfWidth, Math.min(widthUm * MICROMETER_TO_CANVAS_PIXEL - halfWidth, desiredX));
          let y = Math.max(text.height + 2 / scale, (y1 + y2) / 2 + (guide.axis === "y" ? text.height / 2 : -5 / scale));
          // Separate only labels that would collide at the same reference.
          while (labels.some((label) => Math.abs(label.x - x) < text.width && Math.abs(label.y - y) < text.height)) {
            y += 14 / scale;
          }
          y = Math.min(heightUm * MICROMETER_TO_CANVAS_PIXEL - 2 / scale, y);
          text.position.set(x, y);
          labels.push({ x, y });
          const background = new Graphics().rect(x - halfWidth - 3 / scale, y - text.height - 1 / scale, text.width + 6 / scale, text.height + 2 / scale)
            .fill({ color: 0xffffff, alpha: 0.9 });
          background.eventMode = "none";
          content.addChild(background, text);
        }
        // Scene units scale with Zoom; this stroke stays one logical screen pixel.
        drawing.stroke({ color: SNAP_COLOR, width: 1 / scale });
        content.addChild(drawing);
      }
    },
  };
}

function consolidateGuides(guides: readonly FrameSnapGuide[]) {
  const result: FrameSnapGuide[] = [];
  for (const guide of guides) {
    const match = result.find((other) => guide.measurementUm === null && other.measurementUm === null && other.axis === guide.axis &&
      (guide.axis === "x" ? other.x1 === guide.x1 : other.y1 === guide.y1));
    if (match) {
      match.x1 = Math.min(match.x1, guide.x1);
      match.y1 = Math.min(match.y1, guide.y1);
      match.x2 = Math.max(match.x2, guide.x2);
      match.y2 = Math.max(match.y2, guide.y2);
    } else if (!result.some((other) => other.x1 === guide.x1 && other.x2 === guide.x2 && other.y1 === guide.y1 && other.y2 === guide.y2 && other.measurementUm === guide.measurementUm)) {
      result.push({ ...guide });
    }
  }
  return result;
}
