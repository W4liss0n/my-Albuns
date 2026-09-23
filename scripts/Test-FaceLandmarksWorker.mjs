import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { mock, test } from "node:test";
import assert from "node:assert/strict";

const source = readFileSync(new URL("../public/models/faceLandmarks.worker.js", import.meta.url), "utf8");

function harness(detect, options = {}) {
  const postMessage = mock.fn();
  const image = { width: 1000, height: 1600, close: mock.fn() };
  const self = { postMessage };
  const createCanvas = mock.fn();
  class Canvas {
    constructor(width, height) { this.width = width; this.height = height; createCanvas(); }
    getContext() { return { drawImage: (_image, ...rect) => { this.rect = rect; } }; }
  }
  const detector = { detect: mock.fn(detect) };
  const createFromOptions = mock.fn(options.createFromOptions ?? (async () => detector));
  runInNewContext(source, { self, importScripts: mock.fn(), OffscreenCanvas: Canvas, performance, setTimeout: options.setTimeout ?? setTimeout,
    Vision: { FilesetResolver: { forVisionTasks: async () => ({}) }, FaceLandmarker: { createFromOptions } },
  });
  return { image, detector, createCanvas, createFromOptions, postMessage,
    message: () => JSON.parse(JSON.stringify(postMessage.mock.calls[0].arguments[0])),
    messages: () => postMessage.mock.calls.map(call => JSON.parse(JSON.stringify(call.arguments[0]))),
    run: (id = 1, bitmap = image) => self.onmessage({ data: { id, image: bitmap } }),
    cancel: id => self.onmessage({ data: { cancel: id } }),
  };
}

function facesInRegion(input, faces) {
  if (!input.rect) return [];
  const [x, y, width, height] = input.rect;
  return faces.filter(face => face.every(p => p.x * 1000 >= x && p.x * 1000 <= x + width && p.y * 1600 >= y && p.y * 1600 <= y + height))
    .map(face => face.map(p => ({ x: (p.x * 1000 - x) / width, y: (p.y * 1600 - y) / height, z: p.z * 1000 / width })));
}

for (const maxRegionWidth of [250, 125]) test(`detailed search finds small faces needing regions of width ${maxRegionWidth}`, async () => {
  const face = [{ x: .31, y: .21, z: 0 }, { x: .34, y: .24, z: 0 }];
  const h = harness(input => ({ faceLandmarks: input.rect?.[2] <= maxRegionWidth ? facesInRegion(input, [face]) : [] }));
  await h.run();
  assert.equal(h.message().faces.length, 1);
});

test("the per-region model limit does not truncate the combined photo to eight faces", async () => {
  const faces = Array.from({ length: 12 }, (_, i) => {
    const x = .1 + i % 4 * .22, y = .1 + Math.floor(i / 4) * .3;
    return [{ x, y, z: 0 }, { x: x + .06, y: y + .06, z: 0 }];
  });
  const h = harness(input => ({ faceLandmarks: facesInRegion(input, faces) }));
  await h.run();
  assert.equal(h.message().faces.length, 12);
});

test("regional search supplements a partial full-image result", async () => {
  const face = [{ x: .4, y: .2, z: -.1 }, { x: .6, y: .4, z: .1 }];
  const small = [{ x: .05, y: .05, z: 0 }, { x: .1, y: .1, z: 0 }];
  const h = harness(input => ({ faceLandmarks: !input.rect ? [face] : facesInRegion(input, [face, small]) }));
  await h.run();
  assert.equal(h.message().faces.length, 2);
  assert.ok(Math.abs(h.message().faces[1][0].x - face[0].x) < 1e-8);
  assert.ok(h.detector.detect.mock.callCount() > 1);
  assert.equal(h.image.close.mock.callCount(), 1);
});

test("small faces missed in the full image are mapped back from overlapping regions without duplicates", async () => {
  const faces = [
    [{ x: .45, y: .38, z: -.01 }, { x: .55, y: .48, z: .01 }],
    [{ x: .1, y: .7, z: -.01 }, { x: .2, y: .8, z: .01 }],
  ];
  const h = harness((input) => {
    return { faceLandmarks: facesInRegion(input, faces) };
  });
  await h.run();
  const output = h.message();
  assert.equal(output.faces.length, 2);
  for (let f = 0; f < faces.length; f++) for (let p = 0; p < 2; p++) for (const axis of ["x", "y", "z"]) {
    assert.ok(Math.abs(output.faces[f][p][axis] - faces[f][p][axis]) < 1e-8);
  }
  assert.ok(h.detector.detect.mock.callCount() <= 284 + 2 * faces.length);
  assert.equal(h.image.close.mock.callCount(), 1);
});

test("an empty image finishes after the bounded search and preserves the no-face result", async () => {
  const h = harness(() => ({ faceLandmarks: [] }));
  await h.run();
  assert.deepEqual(h.message(), { id: 1, faces: [] });
  assert.ok(h.detector.detect.mock.callCount() <= 284);
  assert.equal(h.image.close.mock.callCount(), 1);
});

test("a background pattern detected in one tile is rejected when the face crop cannot confirm it", async () => {
  const h = harness(input => ({ faceLandmarks: input.rect?.[2] === 500
    ? [[{ x: .4, y: .4, z: 0 }, { x: .5, y: .5, z: 0 }]] : [] }));
  await h.run();
  assert.deepEqual(h.message(), { id: 1, faces: [] });
});

test("a fine-only candidate must remain a face in both close and contextual crops", async () => {
  const face = [{ x: .31, y: .21, z: 0 }, { x: .34, y: .24, z: 0 }];
  const h = harness(input => ({ faceLandmarks: !input.rect || input.rect[2] > 125
    || input.rect[2] >= 55 && input.rect[2] <= 65 ? [] : facesInRegion(input, [face]) }));
  await h.run();
  assert.deepEqual(h.message(), { id: 1, faces: [] });
});

test("an established face is retained when only the contextual crop can confirm it", async () => {
  const face = [{ x: .31, y: .21, z: 0 }, { x: .34, y: .24, z: 0 }];
  const h = harness(input => ({ faceLandmarks: !input.rect ? [face]
    : input.rect[2] >= 55 && input.rect[2] <= 65 ? [] : facesInRegion(input, [face]) }));
  await h.run();
  assert.equal(h.message().faces.length, 1);
});

test("the returned eye landmarks come from the confirmed face view", async () => {
  const coarse = [{ x: .45, y: .38, z: -.01 }, { x: .55, y: .48, z: .01 }];
  const refined = [{ x: .43, y: .35, z: -.02 }, { x: .56, y: .48, z: .02 }];
  const h = harness(input => ({ faceLandmarks: !input.rect ? [coarse]
    : facesInRegion(input, [input.rect[2] >= 290 && input.rect[2] <= 310 ? refined : coarse]) }));
  await h.run();
  assert.equal(h.message().faces.length, 1);
  assert.ok(Math.abs(h.message().faces[0][0].y - refined[0].y) < 1e-8);
});

test("invalid landmarks are not selectable and do not prevent bitmap cleanup", async () => {
  const h = harness(() => ({ faceLandmarks: [[], [{ x: NaN, y: .5, z: 0 }], [{ x: 1.1, y: .5, z: 0 }]] }));
  await h.run();
  assert.deepEqual(h.message(), { id: 1, faces: [] });
  assert.equal(h.image.close.mock.callCount(), 1);
});

test("detector failures reach the caller and release the input bitmap", async () => {
  const h = harness(() => { throw new Error("detector unavailable"); });
  await h.run();
  assert.match(h.message().error, /detector unavailable/);
  assert.equal(h.image.close.mock.callCount(), 1);
});

test("cancelling before the model loads closes the bitmap without scanning or replying", async () => {
  let resolveModel;
  const model = new Promise(resolve => { resolveModel = resolve; });
  const detector = { detect: mock.fn(() => ({ faceLandmarks: [] })) };
  const h = harness(() => ({ faceLandmarks: [] }), { createFromOptions: () => model });
  const first = h.run();
  await h.cancel(1);
  resolveModel(detector);
  await first;
  assert.equal(detector.detect.mock.callCount(), 0);
  assert.deepEqual(h.messages(), []);
  assert.equal(h.image.close.mock.callCount(), 1);
});

test("cancelling active regional analysis stops at the next checkpoint and preserves the model for another job", async () => {
  let resumeFirst;
  let holdFirst = true;
  const h = harness(() => ({ faceLandmarks: [] }), { setTimeout: callback => {
    if (holdFirst) { holdFirst = false; resumeFirst = callback; }
    else setTimeout(callback, 0);
  } });
  const abandoned = h.run(1);
  for (let i = 0; i < 40 && !resumeFirst; i++) await Promise.resolve();
  assert.ok(resumeFirst, "analysis reached a yielded region checkpoint");
  const callsBeforeCancel = h.detector.detect.mock.callCount();
  await h.cancel(1);
  const nextImage = { width: 1000, height: 1600, close: mock.fn() };
  const next = h.run(2, nextImage);
  resumeFirst();
  await Promise.all([abandoned, next]);
  assert.equal(h.detector.detect.mock.callCount() - callsBeforeCancel, 284);
  assert.deepEqual(h.messages(), [{ id: 2, faces: [] }]);
  assert.equal(h.image.close.mock.callCount(), 1);
  assert.equal(nextImage.close.mock.callCount(), 1);
  assert.equal(h.createFromOptions.mock.callCount(), 1);
});
