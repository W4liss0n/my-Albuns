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
          if (entry.desired && !this.destroyed) this.onError();
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
