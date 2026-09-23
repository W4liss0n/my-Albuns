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
self.onmessage = async ({ data }) => {
  try {
    const faces = (await landmarker()).detect(data.image).faceLandmarks;
    self.postMessage({ id: data.id, faces: faces.map((face) => face.map(({ x, y, z }) => ({ x, y, z }))) });
  } catch (error) {
    detector = null;
    self.postMessage({ id: data.id, error: error instanceof Error ? error.message : String(error) });
  } finally {
    data.image.close();
  }
};
