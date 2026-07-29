import { beforeEach, expect, test, vi } from "vitest";

import { ViewportTexturePool } from "./viewportTexturePool";

const assets = vi.hoisted(() => ({
  loads: [] as string[],
  pending: [] as Array<{
    url: string;
    resolve(texture: object): void;
  }>,
  unloads: [] as string[],
}));

vi.mock("pixi.js", () => ({
  Assets: {
    load: vi.fn(
      (url: string) =>
        new Promise<object>((resolve) => {
          assets.loads.push(url);
          assets.pending.push({ url, resolve });
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
