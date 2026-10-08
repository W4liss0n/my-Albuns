import {
  Container,
  type FederatedPointerEvent,
  type FederatedWheelEvent,
  Graphics,
} from "pixi.js";
import { expect, test, vi } from "vitest";

import type { PhotoTransformDelta } from "./albumCanvasContract";
import type { PhotoRenderNode } from "./albumCanvasRenderNodes";
import { interactiveComposition } from "./albumCanvasTestFixtures";
import { MICROMETER_TO_CANVAS_PIXEL } from "./canvasGeometry";
import { createPhotoGeometry } from "./photoGeometry";
import { PhotoInteractionSession } from "./photoInteractionSession";

// Real Pixi Containers: destroy() nulls their position and scale, so touching
// a destroyed node throws, as it does in the application.
function createPhotoNode(): PhotoRenderNode {
  const placement = interactiveComposition.sheets[0].frames[0].photo!.placement;
  const geometry = createPhotoGeometry(placement, MICROMETER_TO_CANVAS_PIXEL);
  const frame = new Container();
  const layer = new Container();
  const outsideLayer = new Container();
  const thirdsGuides = new Graphics();
  frame.addChild(outsideLayer, layer, thirdsGuides);
  for (const preview of [layer, outsideLayer]) {
    preview.position.set(geometry.current.center.x, geometry.current.center.y);
  }
  outsideLayer.visible = false;
  thirdsGuides.visible = false;
  return {
    frameId: "frame-001",
    createDragPreview: () => {
      throw new Error("not used");
    },
    layer,
    outsideLayer,
    thirdsGuides,
    geometry,
    baseZoom: placement.currentZoom,
    baseScaleX: 1,
    originalX: layer.x,
    originalY: layer.y,
    pan: placement.currentPan,
  };
}

function createSession() {
  const photoNodes = new Map<string, PhotoRenderNode>();
  const input = {
    photoZoomPreview: null,
    revision: 1,
    onTransformPreview: vi.fn(),
    onTransformCommit: vi.fn(async (_delta: PhotoTransformDelta) => true),
  };
  const session = new PhotoInteractionSession(photoNodes, () => ({
    input,
    projectGeneration: 1,
    canvasScale: 1,
  }));
  return { session, photoNodes, input };
}

const pointer = (x: number) =>
  ({ global: { x, y: 0 } }) as FederatedPointerEvent;
const wheel = () =>
  ({ deltaY: -100, preventDefault: vi.fn() }) as unknown as FederatedWheelEvent;

// A Sheet rebuild can destroy the dragged node before the scene hands the
// gesture to its replacement, e.g. a render flushed while the press lands.
test.each<{
  name: string;
  run(session: PhotoInteractionSession, replacement: PhotoRenderNode): void;
}>([
  {
    name: "a pointer move",
    run: (session) => session.handlePointerMove(pointer(30)),
  },
  { name: "the release", run: (session) => session.finishPan(pointer(30)) },
  { name: "a cancel", run: (session) => session.cancelPan() },
  {
    name: "a wheel step",
    run: (session, replacement) => session.handleWheel(replacement, wheel()),
  },
  { name: "a reset", run: (session) => session.reset() },
])("forgets an Alt-Pan whose node was destroyed on $name", ({ run }) => {
  const { session, photoNodes, input } = createSession();
  const dragged = createPhotoNode();
  photoNodes.set(dragged.frameId, dragged);
  session.startPan(dragged, pointer(0));
  session.handlePointerMove(pointer(20));
  photoNodes.delete(dragged.frameId);
  dragged.layer.parent!.destroy({ children: true });
  expect(() => dragged.layer.position.set(0, 0)).toThrow(TypeError);
  const replacement = createPhotoNode();
  photoNodes.set(replacement.frameId, replacement);

  expect(() => run(session, replacement)).not.toThrow();
  expect(() => session.finishPan(pointer(40))).not.toThrow();
  expect(input.onTransformCommit).not.toHaveBeenCalled();

  // No drag is left behind: a wheel step zooms the replacement on its own.
  session.handleWheel(replacement, wheel());
  expect(replacement.layer.scale.y).toBeCloseTo(1.06, 9);
  expect(replacement.layer.x).toBeCloseTo(replacement.originalX, 9);
  expect(replacement.outsideLayer.visible).toBe(false);
});
