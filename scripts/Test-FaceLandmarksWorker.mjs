import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { mock, test } from "node:test";
import assert from "node:assert/strict";

const source = readFileSync(new URL("../public/models/faceLandmarks.worker.js", import.meta.url), "utf8");

function harness(detect) {
  const postMessage = mock.fn();
  const image = { width: 1000, height: 1600, close: mock.fn() };
  const self = { postMessage };
  const createCanvas = mock.fn();
  class Canvas {
    constructor(width, height) { this.width = width; this.height = height; createCanvas(); }
    getContext() { return { drawImage: (_image, ...rect) => { this.rect = rect; } }; }
  }
  const detector = { detect: mock.fn(detect) };
  runInNewContext(source, { self, importScripts: mock.fn(), OffscreenCanvas: Canvas,
    Vision: { FilesetResolver: { forVisionTasks: async () => ({}) }, FaceLandmarker: { createFromOptions: async () => detector } },
  });
  return { image, detector, createCanvas,
    message: () => JSON.parse(JSON.stringify(postMessage.mock.calls[0].arguments[0])),
    run: () => self.onmessage({ data: { id: 1, image } }),
  };
}

test("a face found in the full image avoids extra passes", async () => {
  const face = [{ x: .4, y: .2, z: -.1 }, { x: .6, y: .4, z: .1 }];
  const h = harness(() => ({ faceLandmarks: [face] }));
  await h.run();
  assert.deepEqual(h.message(), { id: 1, faces: [face] });
  assert.equal(h.detector.detect.mock.callCount(), 1);
  assert.equal(h.createCanvas.mock.callCount(), 0);
  assert.equal(h.image.close.mock.callCount(), 1);
});

test("small faces missed in the full image are mapped back from overlapping regions without duplicates", async () => {
  const faces = [
    [{ x: .45, y: .38, z: -.01 }, { x: .55, y: .48, z: .01 }],
    [{ x: .1, y: .7, z: -.01 }, { x: .2, y: .8, z: .01 }],
  ];
  const h = harness((input) => {
    if (!input.rect) return { faceLandmarks: [] };
    const [x, y, width, height] = input.rect;
    return { faceLandmarks: faces.filter(face => face.every(p => p.x * 1000 >= x && p.x * 1000 <= x + width && p.y * 1600 >= y && p.y * 1600 <= y + height))
      .map(face => face.map(p => ({ x: (p.x * 1000 - x) / width, y: (p.y * 1600 - y) / height, z: p.z * 1000 / width }))) };
  });
  await h.run();
  const output = h.message();
  assert.equal(output.faces.length, 2);
  for (let f = 0; f < faces.length; f++) for (let p = 0; p < 2; p++) for (const axis of ["x", "y", "z"]) {
    assert.ok(Math.abs(output.faces[f][p][axis] - faces[f][p][axis]) < 1e-8);
  }
  assert.equal(h.detector.detect.mock.callCount(), 10);
  assert.equal(h.createCanvas.mock.callCount(), 1);
  assert.equal(h.image.close.mock.callCount(), 1);
});

test("an empty image finishes after the bounded search and preserves the no-face result", async () => {
  const h = harness(() => ({ faceLandmarks: [] }));
  await h.run();
  assert.deepEqual(h.message(), { id: 1, faces: [] });
  assert.equal(h.detector.detect.mock.callCount(), 10);
  assert.equal(h.image.close.mock.callCount(), 1);
});
