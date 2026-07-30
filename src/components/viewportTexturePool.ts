import { Assets, type Texture } from "pixi.js";

interface TextureEntry {
  desired: boolean;
  failed: boolean;
  operation: Promise<void> | null;
  texture?: Texture;
}

export class ViewportTexturePool {
  private readonly entries = new Map<string, TextureEntry>();
  private destroyed = false;

  constructor(
    private readonly onChange: () => void,
    private readonly onError: () => void = () => undefined,
  ) {}

  sync(urls: Iterable<string>) {
    if (this.destroyed) return;
    const desired = new Set(urls);

    for (const [url, entry] of this.entries) {
      const nextDesired = desired.has(url);
      if (nextDesired && !entry.desired) entry.failed = false;
      entry.desired = nextDesired;
    }
    for (const url of desired) {
      if (!this.entries.has(url)) {
        this.entries.set(url, {
          desired: true,
          failed: false,
          operation: null,
        });
      }
    }
    for (const [url, entry] of this.entries) {
      this.reconcile(url, entry);
    }
  }

  get(url: string) {
    return this.entries.get(url)?.texture;
  }

  textureSize(url: string) {
    const texture = this.entries.get(url)?.texture;
    return texture ? exactTextureSize(texture) : null;
  }

  isSettled() {
    for (const entry of this.entries.values()) {
      if (entry.desired && !entry.texture && !entry.failed) {
        return false;
      }
    }
    return true;
  }

  residency() {
    let count = 0;
    let pixelCount = 0;
    for (const entry of this.entries.values()) {
      if (!entry.texture) continue;
      count += 1;
      const size = exactTextureSize(entry.texture);
      if (size) pixelCount += size.widthPx * size.heightPx;
    }
    return { count, pixelCount };
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    for (const [url, entry] of this.entries) {
      entry.desired = false;
      this.reconcile(url, entry);
    }
  }

  private reconcile(url: string, entry: TextureEntry) {
    if (entry.operation) return;
    if (!entry.desired && !entry.texture) {
      this.entries.delete(url);
      return;
    }
    if (entry.desired && !entry.texture && !entry.failed) {
      entry.operation = Assets.load<Texture>(url)
        .then((texture) => {
          entry.texture = texture;
          if (entry.desired && !this.destroyed) this.onChange();
        })
        .catch(() => {
          entry.failed = true;
          if (entry.desired && !this.destroyed) {
            this.onError();
            this.onChange();
          }
        })
        .finally(() => {
          entry.operation = null;
          this.reconcile(url, entry);
        });
      return;
    }
    if (!entry.desired && entry.texture) {
      entry.texture = undefined;
      entry.operation = Assets.unload(url)
        .catch(() => {
          if (!this.destroyed) this.onError();
        })
        .finally(() => {
          entry.operation = null;
          this.reconcile(url, entry);
        });
    }
  }
}

function exactTextureSize(texture: Texture) {
  const source = texture.source as
    | {
        pixelWidth?: unknown;
        pixelHeight?: unknown;
      }
    | undefined;
  const width = source?.pixelWidth;
  const height = source?.pixelHeight;
  if (
    typeof width !== "number" ||
    typeof height !== "number" ||
    !Number.isSafeInteger(width) ||
    !Number.isSafeInteger(height) ||
    width <= 0 ||
    height <= 0
  ) {
    return null;
  }
  const pixels = width * height;
  return Number.isSafeInteger(pixels)
    ? { widthPx: width, heightPx: height }
    : null;
}
