import { beforeEach, expect, test, vi } from "vitest";

import { ViewportTexturePool } from "./viewportTexturePool";
import { registerMediaPreviewImage } from "../application/mediaPreviewImages";

const assets = vi.hoisted(() => ({
  loads: [] as string[],
  pending: [] as Array<{
    url: string;
    resolve(texture: object): void;
    reject(reason: unknown): void;
  }>,
  unloads: [] as string[],
}));

vi.mock("pixi.js", () => ({
  Texture: { from: vi.fn((image: HTMLImageElement) => ({ source: { resource: image }, destroy: vi.fn() })) },
  Assets: {
    load: vi.fn(
      (url: string) =>
        new Promise<object>((resolve, reject) => {
          assets.loads.push(url);
          assets.pending.push({ url, resolve, reject });
        }),
    ),
    unload: vi.fn(async (url: string) => {
      assets.unloads.push(url);
    }),
  },
}));

beforeEach(() => {
  assets.loads.length = 0;
  assets.pending.length = 0;
  assets.unloads.length = 0;
});

test("owns a viewport texture independently from the mounted thumbnail and other Canvases", () => {
  const url = "http://myalbuns-cache.localhost/photo.jpg";
  const image = document.createElement("img");
  image.crossOrigin = "anonymous";
  image.src = url;
  Object.defineProperties(image, {
    complete: { value: true }, naturalWidth: { value: 1_200 },
    naturalHeight: { value: 800 }, currentSrc: { value: url },
  });
  const release = registerMediaPreviewImage(url, image);
  const first = new ViewportTexturePool(vi.fn());
  const second = new ViewportTexturePool(vi.fn());
  try {
    first.sync([url]);
    second.sync([url]);
    const texture = first.get(url)!;
    const other = second.get(url)!;
    expect(texture).toBeDefined();
    expect(other).not.toBe(texture);
    expect(assets.loads).toEqual([]);
    release();
    expect(first.get(url)).toBe(texture);
    first.sync([]);
    expect(texture.destroy).toHaveBeenCalledExactlyOnceWith(true);
    expect(other.destroy).not.toHaveBeenCalled();
    expect(first.get(url)).toBeUndefined();
    expect(image.src).toBe(url);
    expect(assets.unloads).toEqual([]);
  } finally {
    release();
    first.destroy();
    second.destroy();
  }
});

test("loads only desired viewport textures and unloads them when released", async () => {
  const onChange = vi.fn();
  const pool = new ViewportTexturePool(onChange);
  const texture = { label: "preview-texture" };

  pool.sync(["asset://cache/photo-a.jpg"]);
  expect(assets.loads).toEqual(["asset://cache/photo-a.jpg"]);
  expect(pool.get("asset://cache/photo-a.jpg")).toBeUndefined();

  assets.pending[0].resolve(texture);
  await vi.waitFor(() => {
    expect(pool.get("asset://cache/photo-a.jpg")).toBe(texture);
  });
  expect(onChange).toHaveBeenCalledOnce();

  pool.sync([]);
  await vi.waitFor(() => {
    expect(assets.unloads).toEqual(["asset://cache/photo-a.jpg"]);
  });
  expect(pool.get("asset://cache/photo-a.jpg")).toBeUndefined();
});

test("does not retain a texture whose sheet left the viewport while loading", async () => {
  const onChange = vi.fn();
  const pool = new ViewportTexturePool(onChange);

  pool.sync(["asset://cache/photo-a.jpg"]);
  pool.sync([]);
  assets.pending[0].resolve({ label: "stale-texture" });

  await vi.waitFor(() => {
    expect(assets.unloads).toEqual(["asset://cache/photo-a.jpg"]);
  });
  expect(pool.get("asset://cache/photo-a.jpg")).toBeUndefined();
  expect(onChange).not.toHaveBeenCalled();
});

test("reports a desired texture failure without discarding loaded textures", async () => {
  const onChange = vi.fn();
  const onError = vi.fn();
  const pool = new ViewportTexturePool(onChange, onError);

  pool.sync([
    "asset://cache/photo-a.jpg",
    "asset://cache/photo-b.jpg",
  ]);

  const texture = { label: "preview-texture" };
  assets.pending[0].resolve(texture);
  await vi.waitFor(() => {
    expect(pool.get("asset://cache/photo-a.jpg")).toBe(texture);
  });

  assets.pending[1].reject(new Error("invalid texture"));
  await vi.waitFor(() => {
    expect(onError).toHaveBeenCalledOnce();
  });
  expect(onChange).toHaveBeenCalledTimes(2);
  expect(pool.get("asset://cache/photo-a.jpg")).toBe(texture);
  expect(assets.loads).toEqual([
    "asset://cache/photo-a.jpg",
    "asset://cache/photo-b.jpg",
  ]);
});
