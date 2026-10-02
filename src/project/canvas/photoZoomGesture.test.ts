import { expect, test } from "vitest";

import {
  advancePhotoZoomGesture,
  finishPhotoZoomGesture,
} from "./photoZoomGesture";

const zoomRange = { minimum: 1, maximum: 4 };

test("groups consecutive wheel events into one Photo Zoom commit", () => {
  let transition = advancePhotoZoomGesture(null, {
    frameId: "frame-a",
    baseZoom: 1,
    zoomRange,
    wheelDeltaY: -100,
  });

  expect(transition.interruptedCommit).toBeNull();
  expect(transition.previewZoom).toBeCloseTo(1.12, 6);

  transition = advancePhotoZoomGesture(transition.gesture, {
    frameId: "frame-a",
    baseZoom: 1,
    zoomRange,
    wheelDeltaY: -100,
  });
  transition = advancePhotoZoomGesture(transition.gesture, {
    frameId: "frame-a",
    baseZoom: 1,
    zoomRange,
    wheelDeltaY: -100,
  });

  expect(transition.previewZoom).toBeCloseTo(1.36, 6);
  const commit = finishPhotoZoomGesture(transition.gesture);
  expect(commit?.frameId).toBe("frame-a");
  expect(commit?.delta).toBeCloseTo(0.36, 6);
});

test("finishes the previous Frame before starting another gesture", () => {
  const first = advancePhotoZoomGesture(null, {
    frameId: "frame-a",
    baseZoom: 1,
    zoomRange,
    wheelDeltaY: -100,
  });
  const second = advancePhotoZoomGesture(first.gesture, {
    frameId: "frame-b",
    baseZoom: 2,
    zoomRange,
    wheelDeltaY: 100,
  });

  expect(second.interruptedCommit?.frameId).toBe("frame-a");
  expect(second.interruptedCommit?.delta).toBeCloseTo(0.12, 6);
  expect(second.gesture.frameId).toBe("frame-b");
  expect(second.previewZoom).toBeCloseTo(1.88, 6);
});

test("keeps Photo Zoom within the domain range", () => {
  const maximum = advancePhotoZoomGesture(null, {
    frameId: "frame-a",
    baseZoom: 3.95,
    zoomRange,
    wheelDeltaY: -1_000,
  });
  const minimum = advancePhotoZoomGesture(null, {
    frameId: "frame-b",
    baseZoom: 1.05,
    zoomRange,
    wheelDeltaY: 1_000,
  });

  expect(maximum.previewZoom).toBe(4);
  expect(minimum.previewZoom).toBe(1);
});
