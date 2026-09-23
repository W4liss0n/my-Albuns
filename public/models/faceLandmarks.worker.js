/* MediaPipe Tasks Vision 1.0.1; loaded as a classic worker to keep Vite's dev
   module transform away from MediaPipe's runtime WASM loader. */
importScripts("/models/vision_bundle.js");
let detector = null;
function landmarker() {
  if (!detector) {
    detector = Vision.FilesetResolver.forVisionTasks("/models/wasm")
      .then((files) => Vision.FaceLandmarker.createFromOptions(files, {
        baseOptions: { modelAssetPath: "/models/face_landmarker.task", delegate: "CPU" },
        runningMode: "IMAGE", numFaces: 8,
        minFaceDetectionConfidence: 0.55, minFacePresenceConfidence: 0.55,
      }));
  }
  return detector;
}

function faceBounds(face) {
  const xs = face.map((point) => point.x), ys = face.map((point) => point.y);
  return { left: Math.min(...xs), top: Math.min(...ys), right: Math.max(...xs), bottom: Math.max(...ys) };
}

function sameFace(a, b) {
  const intersection = Math.max(0, Math.min(a.right, b.right) - Math.max(a.left, b.left))
    * Math.max(0, Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top));
  const smallerArea = Math.min((a.right - a.left) * (a.bottom - a.top), (b.right - b.left) * (b.bottom - b.top));
  return smallerArea > 0 && intersection / smallerArea > .6;
}

function detectFaces(landmarker, image) {
  const found = [];
  function candidates(faces, left, top, width, height) {
    const result = [];
    for (const face of faces) {
      if (!face.length || face.some(({ x, y, z }) => !Number.isFinite(x + y + z) || x < 0 || y < 0 || x > 1 || y > 1)) continue;
      const local = faceBounds(face);
      const margin = Math.min(local.left, local.top, 1 - local.right, 1 - local.bottom);
      const mapped = face.map(({ x, y, z }) => ({ x: (left + x * width) / image.width, y: (top + y * height) / image.height, z: z * width / image.width }));
      const bounds = faceBounds(mapped);
      if (bounds.right > bounds.left && bounds.bottom > bounds.top) result.push({ face: mapped, bounds, margin });
    }
    return result;
  }
  function merge(items, fineOnly = false) {
    for (const candidate of items) {
      const duplicate = found.findIndex((item) => sameFace(item.bounds, candidate.bounds));
      candidate.fineOnly = fineOnly;
      if (duplicate < 0) found.push(candidate);
      else if (candidate.margin > found[duplicate].margin) {
        candidate.fineOnly = candidate.fineOnly && found[duplicate].fineOnly;
        found[duplicate] = candidate;
      }
    }
  }
  merge(candidates(landmarker.detect(image).faceLandmarks, 0, 0, image.width, image.height));

  // Small faces need a closer view even when a larger face was already found.
  // Three fixed scales with 50% overlap bound discovery to 284 passes.
  const canvas = new OffscreenCanvas(1, 1);
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Não foi possível preparar a análise da foto.");
  function region(left, top, width, height) {
    const scale = Math.min(1, 800 / Math.max(width, height));
    const canvasWidth = Math.max(1, Math.round(width * scale));
    const canvasHeight = Math.max(1, Math.round(height * scale));
    // Setting either dimension clears and reallocates the surface, even when
    // the value is unchanged. Most tiles at a scale share identical dimensions.
    if (canvas.width !== canvasWidth) canvas.width = canvasWidth;
    if (canvas.height !== canvasHeight) canvas.height = canvasHeight;
    // drawImage preserves displayed EXIF orientation when cropping a bitmap.
    context.drawImage(image, left, top, width, height, 0, 0, canvas.width, canvas.height);
    return candidates(landmarker.detect(canvas).faceLandmarks, left, top, width, height);
  }
  for (const divisions of [2, 4, 8]) {
    const width = Math.ceil(image.width / divisions), height = Math.ceil(image.height / divisions);
    const steps = 2 * (divisions - 1);
    for (let row = 0; row <= steps; row++) for (let column = 0; column <= steps; column++) {
      const left = Math.round((image.width - width) * column / steps);
      const top = Math.round((image.height - height) * row / steps);
      merge(region(left, top, width, height), divisions === 8);
    }
  }
  // Tile edges and background patterns can produce isolated false positives.
  // Confirm context for every face. The finest tiles need a second close view:
  // tiny patterns in clothes/floors otherwise survive a single confirmation.
  const confirmed = found.flatMap(({ bounds, fineOnly }) => {
    let refined;
    for (const expansion of (fineOnly ? [2, 3] : [3])) {
      const width = (bounds.right - bounds.left) * image.width;
      const height = (bounds.bottom - bounds.top) * image.height;
      const left = Math.max(0, Math.floor(bounds.left * image.width - width * (expansion - 1) / 2));
      const top = Math.max(0, Math.floor(bounds.top * image.height - height * (expansion - 1) / 2));
      const cropWidth = Math.min(image.width - left, Math.ceil(width * expansion));
      const cropHeight = Math.min(image.height - top, Math.ceil(height * expansion));
      refined = region(left, top, cropWidth, cropHeight).find(item => sameFace(item.bounds, bounds));
      if (!refined) return [];
    }
    // Use the full face view for the eye coordinates, not a tile-edge estimate.
    return [refined];
  });
  // numFaces limits a model pass, not the number of people in the entire photo.
  return confirmed.sort((a, b) => a.bounds.top - b.bounds.top || a.bounds.left - b.bounds.left).map((item) => item.face);
}

self.onmessage = async ({ data }) => {
  try {
    const faces = detectFaces(await landmarker(), data.image);
    self.postMessage({ id: data.id, faces: faces.map((face) => face.map(({ x, y, z }) => ({ x, y, z }))) });
  } catch (error) {
    detector = null;
    self.postMessage({ id: data.id, error: error instanceof Error ? error.message : String(error) });
  } finally {
    data.image.close();
  }
};
