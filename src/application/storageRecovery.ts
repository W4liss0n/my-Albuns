export interface StorageRecovery { id: string; canClearCache: boolean }
export interface StorageRecoveryPort {
  status(owner: string): Promise<StorageRecovery | null>;
  clear(id: string): Promise<boolean>;
}
export class StorageFullError extends Error {}
export const unavailableStorageRecovery: StorageRecoveryPort = {
  status: async () => null, clear: async () => false,
};
export interface StorageFullPresentation {
  kind: "storageFull";
  message: string;
  canClearCache: boolean;
  busy: boolean;
}
export type StorageRecoveryAction = "resumeStorage" | "clearStorageCache" | "cancelStorage";

/** One user gesture authorizes at most one cleanup and one retry. */
export class StorageRecoveryController {
  private generation = 0;
  private current: { recovery: StorageRecovery | null; state: StorageFullPresentation } | null = null;
  constructor(private readonly port: StorageRecoveryPort,
    private readonly present: (state: StorageFullPresentation) => void,
    private readonly resume: (id?: string) => void,
    private readonly cancel: (id?: string) => void) {}

  async open(owner: string, message: string, onlyIfPaused = false) {
    if (this.current) return;
    const generation = ++this.generation;
    this.current = null;
    const recovery = await this.port.status(owner).catch(() => null);
    if (generation !== this.generation) return;
    if (onlyIfPaused && !recovery) return;
    this.current = { recovery, state: { kind: "storageFull", message, canClearCache: recovery?.canClearCache ?? false, busy: false } };
    this.present(this.current.state);
  }

  async act(action: StorageRecoveryAction) {
    const current = this.current;
    if (!current || current.state.busy) return;
    if (action === "cancelStorage") { this.dispose(); this.cancel(current.recovery?.id); return; }
    if (action === "resumeStorage") { this.dispose(); this.resume(current.recovery?.id); return; }
    if (!current.recovery || !current.state.canClearCache) return;
    const generation = this.generation;
    current.state = { ...current.state, busy: true };
    this.present(current.state);
    let freed = false;
    try { freed = await this.port.clear(current.recovery.id); } catch { /* Keep the operation paused. */ }
    if (generation !== this.generation) return;
    if (freed) { this.dispose(); this.resume(current.recovery?.id); return; }
    current.state = { ...current.state, busy: false, canClearCache: false };
    this.present(current.state);
  }

  dispose() { this.generation++; this.current = null; }
}
