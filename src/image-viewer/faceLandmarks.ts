export interface FacePoint { x: number; y: number; z: number }
export type Face = FacePoint[];

let worker: Worker | null = null;
let serial = 0;
const pending = new Map<number, { resolve(faces: Face[]): void; reject(error: Error): void }>();

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

export async function detectFaces(image: HTMLImageElement): Promise<Face[]> {
  const bitmap = await createImageBitmap(image);
  const id = ++serial;
  return new Promise((resolve, reject) => {
    pending.set(id, { resolve, reject });
    getWorker().postMessage({ id, image: bitmap }, [bitmap]);
  });
}

export function faceCenter(face: Face) {
  const points = [face[33], face[263], face[1], face[152]].filter(Boolean);
  return {
    x: points.reduce((sum, point) => sum + point.x, 0) / points.length,
    y: points.reduce((sum, point) => sum + point.y, 0) / points.length,
  };
}
