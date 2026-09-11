import { Assets, Texture } from "pixi.js";
import { loadedMediaPreviewImage } from "../application/mediaPreviewImages";

interface TextureEntry {
  desired: boolean;
  failed: boolean;
  operation: Promise<void> | null;
  texture?: Texture;
  ownsTexture?: boolean;
}

export class ViewportTexturePool {
  private readonly entries = new Map<string, TextureEntry>();
  private destroyed = false;

  constructor(
    private readonly onChange: () => void,
    private readonly onError: () => void = () => undefined,
    private readonly onLoad: (url: string) => void = () => undefined,
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
      const image = loadedMediaPreviewImage(url);
      if (image && isRasterCacheImage(image.currentSrc)) {
        // The panel has already loaded this Cache image. Materialize it in the
        // same update as the Frame, without a second asynchronous Assets load.
        entry.texture = Texture.from(image, true);
        entry.ownsTexture = true;
        this.onLoad(url);
        return;
      }
      entry.operation = Assets.load<Texture>(url)
        .then((texture) => {
          entry.texture = texture;
          if (entry.desired && !this.destroyed) {
            this.onLoad(url);
            this.onChange();
          }
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
      if (entry.ownsTexture) {
        entry.texture.destroy(true);
        this.entries.delete(url);
        return;
      }
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

function isRasterCacheImage(url: string) {
  // Cache artifacts are PNG/JPEG. SVG DOM images can be rasterized by the
  // browser at thumbnail size, so they must keep Pixi's dedicated SVG loader.
  if (url.startsWith("data:")) return /^data:image\/(?:png|jpeg)[;,]/i.test(url);
  return /\.(?:png|jpe?g)$/i.test(new URL(url).pathname);
}
