export interface FacePoint { x: number; y: number; z: number }
export type Face = FacePoint[];

let worker: Worker | null = null;
let serial = 0;
const pending = new Map<number, { resolve(faces: Face[]): void; reject(error: Error): void }>();
const results = new Map<string, Promise<Face[]>>();
const MAX_REUSED_IMAGES = 12;

function getWorker() {
  if (!worker) {
    worker = new Worker("/models/faceLandmarks.worker.js");
    worker.onmessage = ({ data }: MessageEvent<{ id: number; faces?: Face[]; error?: string }>) => {
      const item = pending.get(data.id);
      if (!item) return;
      pending.delete(data.id);
      if (data.error) item.reject(new Error(data.error));
      else item.resolve(data.faces ?? []);
    };
    worker.onerror = () => {
      for (const item of pending.values()) item.reject(new Error("Não foi possível analisar os rostos."));
      pending.clear();
      worker?.terminate();
      worker = null;
    };
  }
  return worker;
}

async function scanFaces(image: HTMLImageElement): Promise<Face[]> {
  const bitmap = await createImageBitmap(image);
  const id = ++serial;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    getWorker().postMessage({ id, image: bitmap }, [bitmap]);
  });
}

export function detectFaces(image: HTMLImageElement): Promise<Face[]> {
  // Cache preview URLs identify immutable Cache publications. A reference can be
  // revisited while choosing a face, and React can observe load and mount at once.
  const key = `${image.currentSrc || image.src}:${image.naturalWidth}x${image.naturalHeight}`;
  if (!key || key.startsWith(":") || key.startsWith("undefined:")) return scanFaces(image);
  const reused = results.get(key);
  if (reused) return reused;
  const scan = scanFaces(image).catch((error: unknown) => {
    if (results.get(key) === scan) results.delete(key);
    throw error;
  });
  results.set(key, scan);
  if (results.size > MAX_REUSED_IMAGES) results.delete(results.keys().next().value!);
  return scan;
}

export function faceBounds(face: Face) {
  let left = Infinity, top = Infinity, right = -Infinity, bottom = -Infinity;
  for (const point of face) {
    if (!Number.isFinite(point.x) || !Number.isFinite(point.y)) continue;
    left = Math.min(left, point.x);
    top = Math.min(top, point.y);
    right = Math.max(right, point.x);
    bottom = Math.max(bottom, point.y);
  }
  return Number.isFinite(left) && right > left && bottom > top ? { left, top, right, bottom } : null;
}
