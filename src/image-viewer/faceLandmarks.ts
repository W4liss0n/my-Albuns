import type { ViewerFace } from "../contracts/generated/ViewerFace";
import type { ViewerFacePoint } from "../contracts/generated/ViewerFacePoint";
export type FacePoint = ViewerFacePoint;
export type Face = ViewerFace;

interface Consumer {
  resolve(faces: Face[]): void;
  reject(error: Error): void;
}
interface Scan {
  id: number;
  key: string | null;
  consumers: Set<Consumer>;
  posted: boolean;
  completed: boolean;
}

let worker: Worker | null = null;
let serial = 0;
const pending = new Map<number, Scan>();
const scans = new Map<string, Scan>();
const results = new Map<string, Face[]>();
const MAX_REUSED_IMAGES = 12;

function cancelled() { return new DOMException("Análise cancelada.", "AbortError"); }

function finish(scan: Scan, faces?: Face[], error?: Error) {
  if (scan.completed) return;
  scan.completed = true;
  pending.delete(scan.id);
  if (scan.key && scans.get(scan.key) === scan) scans.delete(scan.key);
  if (faces && scan.key && scan.consumers.size) {
    results.set(scan.key, faces);
    if (results.size > MAX_REUSED_IMAGES) results.delete(results.keys().next().value!);
  }
  for (const consumer of scan.consumers) {
    if (error) consumer.reject(error);
    else consumer.resolve(faces ?? []);
  }
  scan.consumers.clear();
}

function getWorker() {
  if (!worker) {
    const current = new Worker("/models/faceLandmarks.worker.js");
    worker = current;
    current.onmessage = ({ data }: MessageEvent<{ id: number; faces?: Face[]; error?: string }>) => {
      const scan = pending.get(data.id);
      if (scan) finish(scan, data.faces, data.error ? new Error(data.error) : undefined);
    };
    current.onerror = () => {
      for (const scan of [...pending.values()]) finish(scan, undefined, new Error("Não foi possível analisar os rostos."));
      current.terminate();
      if (worker === current) worker = null;
    };
  }
  return worker;
}

function start(scan: Scan, image: HTMLImageElement) {
  void createImageBitmap(image).then((bitmap) => {
    if (scan.completed) { bitmap.close(); return; }
    try {
      pending.set(scan.id, scan);
      getWorker().postMessage({ id: scan.id, image: bitmap }, [bitmap]);
      scan.posted = true;
    } catch (error) {
      bitmap.close();
      finish(scan, undefined, error instanceof Error ? error : new Error(String(error)));
    }
  }).catch((error: unknown) => finish(scan, undefined, error instanceof Error ? error : new Error(String(error))));
}

/** A lease owns one interest. Releasing the last interest stops queued or active work. */
export function acquireFaces(image: HTMLImageElement): { promise: Promise<Face[]>; release(): void } {
  const source = image.currentSrc || image.src;
  const key = source ? `${source}:${image.naturalWidth}x${image.naturalHeight}` : null;
  if (key && results.has(key)) return { promise: Promise.resolve(results.get(key)!), release() {} };
  let scan = key ? scans.get(key) : undefined;
  if (!scan) {
    scan = { id: ++serial, key, consumers: new Set(), posted: false, completed: false };
    if (key) scans.set(key, scan);
    start(scan, image);
  }
  const active = scan;
  let release = () => undefined;
  const promise = new Promise<Face[]>((resolve, reject) => {
    const consumer: Consumer = { resolve, reject };
    release = () => {
      if (!active.consumers.delete(consumer)) return;
      reject(cancelled());
      if (active.consumers.size) return;
      if (active.posted) {
        try { worker?.postMessage({ cancel: active.id }); } catch { /* A failed worker is already gone. */ }
      }
      finish(active);
    };
    active.consumers.add(consumer);
  });
  return { promise, release: () => release() };
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
