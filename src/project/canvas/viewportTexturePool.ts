import { Assets, Texture } from "pixi.js";
import { loadedMediaPreviewImage } from "../../application/mediaPreviewImages";

interface TextureEntry {
  desired: boolean;
  failed: boolean;
  operation: Promise<void> | null;
  texture?: Texture;
  ownsTexture?: boolean;
  /** False while the texture waits for its turn to reach the GPU. */
  uploaded?: boolean;
}

/** Sends a texture to the GPU at once instead of on its first draw. */
export type TextureUpload = (texture: Texture) => void;

// A 1600 px preview takes 60 to 90 ms to upload on a slow PC. Uploading every
// texture of a newly reached Sheet in one draw froze the window for up to
// 700 ms; past this budget the remaining uploads wait for the next frames.
// At least one texture goes per frame, so a slow upload never stalls the rest.
export const TEXTURE_UPLOAD_FRAME_BUDGET_MS = 12;

export class ViewportTexturePool {
  private readonly entries = new Map<string, TextureEntry>();
  private destroyed = false;
  /** The latest desired URLs, in the caller's priority order. */
  private uploadOrder: readonly string[] = [];
  private uploadSpentMs = 0;
  private uploadFrame = 0;

  constructor(
    private readonly onChange: () => void,
    private readonly onError: () => void = () => undefined,
    private readonly onLoad: (url: string) => void = () => undefined,
    private readonly upload?: TextureUpload,
  ) {}

  /** `urls` come first-needed first: uploads follow that order. */
  sync(urls: Iterable<string>) {
    if (this.destroyed) return;
    const desired = new Set(urls);
    this.uploadOrder = [...desired];

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
    this.uploadWithinBudget();
  }

  get(url: string) {
    const entry = this.entries.get(url);
    return entry?.uploaded === false ? undefined : entry?.texture;
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    cancelAnimationFrame(this.uploadFrame);
    this.uploadFrame = 0;
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
        this.receive(entry, Texture.from(image, true));
        entry.ownsTexture = true;
        this.onLoad(url);
        return;
      }
      entry.operation = Assets.load<Texture>(url)
        .then((texture) => {
          this.receive(entry, texture);
          if (entry.desired && !this.destroyed) {
            this.onLoad(url);
            // A texture still waiting for the GPU reports its change later.
            if (this.uploadWithinBudget() || entry.uploaded !== false) this.onChange();
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
      entry.uploaded = undefined;
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

  private receive(entry: TextureEntry, texture: Texture) {
    entry.texture = texture;
    if (!this.upload) return;
    entry.uploaded = false;
    this.requestUploadFrame();
  }

  /** Returns whether a texture became available. */
  private uploadWithinBudget() {
    if (!this.upload || this.destroyed) return false;
    let uploadedAny = false;
    for (const url of this.uploadOrder) {
      if (this.uploadSpentMs > 0 && this.uploadSpentMs >= TEXTURE_UPLOAD_FRAME_BUDGET_MS) break;
      const entry = this.entries.get(url);
      if (!entry?.desired || !entry.texture || entry.uploaded !== false) continue;
      const started = performance.now();
      this.upload(entry.texture);
      // Never zero, so a frame that uploaded anything counts as started.
      this.uploadSpentMs += Math.max(performance.now() - started, 0.001);
      entry.uploaded = true;
      uploadedAny = true;
    }
    if (this.uploadSpentMs > 0) this.requestUploadFrame();
    return uploadedAny;
  }

  private requestUploadFrame() {
    if (this.uploadFrame === 0 && !this.destroyed) {
      this.uploadFrame = requestAnimationFrame(this.nextUploadFrame);
    }
  }

  private readonly nextUploadFrame = () => {
    this.uploadFrame = 0;
    this.uploadSpentMs = 0;
    if (this.uploadWithinBudget()) this.onChange();
  };
}

function isRasterCacheImage(url: string) {
  // Cache artifacts are PNG/JPEG. SVG DOM images can be rasterized by the
  // browser at thumbnail size, so they must keep Pixi's dedicated SVG loader.
  if (url.startsWith("data:")) return /^data:image\/(?:png|jpeg)[;,]/i.test(url);
  return /\.(?:png|jpe?g)$/i.test(new URL(url).pathname);
}
