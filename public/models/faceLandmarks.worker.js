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
  const faces = landmarker.detect(image).faceLandmarks;
  if (faces.length) return faces;

  // Whole-body portraits can lose small faces when the detector reduces the
  // full frame. Search nine overlapping regions only after an empty first pass.
  const width = Math.ceil(image.width / 2), height = Math.ceil(image.height / 2);
  const scale = Math.min(1, 800 / Math.max(width, height));
  const canvas = new OffscreenCanvas(Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale)));
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Não foi possível preparar a análise da foto.");
  const found = [];
  for (const row of [0, .5, 1]) for (const column of [0, .5, 1]) {
    const left = Math.round((image.width - width) * column);
    const top = Math.round((image.height - height) * row);
    // Rasterize the displayed orientation before detection, including EXIF
    // images whose ImageBitmap crop coordinates may refer to encoded pixels.
    context.drawImage(image, left, top, width, height, 0, 0, canvas.width, canvas.height);
    for (const face of landmarker.detect(canvas).faceLandmarks) {
      if (!face.length || face.some(({ x, y, z }) => !Number.isFinite(x + y + z) || x < 0 || y < 0 || x > 1 || y > 1)) continue;
      const local = faceBounds(face);
      const margin = Math.min(local.left, local.top, 1 - local.right, 1 - local.bottom);
      const mapped = face.map(({ x, y, z }) => ({ x: (left + x * width) / image.width, y: (top + y * height) / image.height, z: z * width / image.width }));
      const bounds = faceBounds(mapped);
      const duplicate = found.findIndex((item) => sameFace(item.bounds, bounds));
      const candidate = { face: mapped, bounds, margin };
      if (duplicate < 0) found.push(candidate);
      else if (margin > found[duplicate].margin) found[duplicate] = candidate;
    }
  }
  return found.sort((a, b) => a.bounds.top - b.bounds.top || a.bounds.left - b.bounds.left).slice(0, 8).map((item) => item.face);
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
