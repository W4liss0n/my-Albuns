import { useCallback, useLayoutEffect, useRef, useState } from "react";
import type { ViewerPreviewState } from "../contracts/generated/ViewerPreviewState";

export function photoIdentity(sessionId: string, mediaId: string, url: string | null): string {
  return `${sessionId}:${mediaId}:${url ?? ""}`;
}

interface VisiblePhoto<T> {
  key: string;
  sessionId: string;
  url: string;
  name: string;
  payload: T;
}

/** Owns the last painted photo until the requested one is measurable, or is terminal. */
export function usePhotoContinuity<T>({ key, sessionId, url, name, state, allowRetain = true }: {
  key: string; sessionId: string; url: string | null; name: string; state: ViewerPreviewState; allowRetain?: boolean;
}) {
  const [visible, setVisible] = useState<VisiblePhoto<T> | null>(null);
  const [failedKey, setFailedKey] = useState<string | null>(null);
  const activeKey = useRef(key);
  activeKey.current = key;
  const ready = visible?.key === key && failedKey !== key;
  const pending = !ready && failedKey !== key && (state === "loading" || state === "ready");
  const retained = allowRetain && pending && visible?.sessionId === sessionId ? visible : null;
  useLayoutEffect(() => {
    if (visible?.key !== key && state !== "ready" && state !== "loading") setVisible(null);
  }, [visible?.key, key, state]);
  const markReady = useCallback((requestKey: string, payload: T) => {
    if (activeKey.current !== requestKey || !url) return;
    setVisible((current) => current?.key === requestKey && current.payload === payload ? current : { key: requestKey, sessionId, url, name, payload });
    setFailedKey(null);
  }, [url, name, sessionId]);
  const markFailed = useCallback((requestKey: string) => {
    if (activeKey.current !== requestKey) return;
    setFailedKey(requestKey);
    setVisible(null);
  }, []);
  return { visible, ready, pending, retained, failed: failedKey === key, markReady, markFailed };
}

export interface Size { width: number; height: number }
export interface Point { x: number; y: number }

export function fitPhoto(natural: Size, space: Size, margin: number, allowUpscale: boolean): Size {
  if (!natural.width || !natural.height) return { width: 0, height: 0 };
  const scale = Math.min(
    allowUpscale ? Number.POSITIVE_INFINITY : 1,
    Math.max(0, space.width - margin * 2) / natural.width,
    Math.max(0, space.height - margin * 2) / natural.height,
  );
  return { width: natural.width * scale, height: natural.height * scale };
}

export function clampPhotoZoom(value: number): number {
  return Math.max(1, Math.min(8, value));
}

export function boundPhotoPan(next: Point, zoom: number, fitted: Size, viewport: Size): Point {
  const maxX = Math.max(0, (fitted.width * zoom - viewport.width) / 2);
  const maxY = Math.max(0, (fitted.height * zoom - viewport.height) / 2);
  return { x: Math.max(-maxX, Math.min(maxX, next.x)), y: Math.max(-maxY, Math.min(maxY, next.y)) };
}
