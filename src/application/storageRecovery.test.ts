import { describe, expect, it, vi } from "vitest";
import { StorageRecoveryController, type StorageFullPresentation } from "./storageRecovery";

describe("storage recovery", () => {
  function fixture(clear = vi.fn(async () => true)) {
    const port = { status: vi.fn(async () => ({ id: "pause", canClearCache: true })), clear };
    const states: StorageFullPresentation[] = [];
    const resume = vi.fn(), cancel = vi.fn();
    const controller = new StorageRecoveryController(port, state => states.push(state), resume, cancel);
    return { port, states, resume, cancel, controller };
  }
  it("waits for actual cleanup, blocks duplicate clicks and retries only once", async () => {
    let finish!: (freed: boolean) => void;
    const f = fixture(vi.fn(() => new Promise<boolean>(resolve => { finish = resolve; })));
    await f.controller.open("export", "Libere espaço.");
    const pending = f.controller.act("clearStorageCache");
    await f.controller.act("clearStorageCache");
    await f.controller.act("resumeStorage");
    await f.controller.act("cancelStorage");
    expect(f.states[f.states.length - 1]?.busy).toBe(true);
    expect(f.port.clear).toHaveBeenCalledTimes(1);
    expect(f.resume).not.toHaveBeenCalled();
    expect(f.cancel).not.toHaveBeenCalled();
    finish(true);
    await pending;
    expect(f.resume).toHaveBeenCalledTimes(1);
  });
  it.each([false, "error"])("keeps waiting when cleanup returns %s", async outcome => {
    const f = fixture(vi.fn(async () => { if (outcome === "error") throw new Error("locked"); return false; }));
    await f.controller.open("cache", "Libere espaço.");
    await f.controller.act("clearStorageCache");
    expect(f.states[f.states.length - 1]).toMatchObject({ busy: false, canClearCache: false });
    expect(f.resume).not.toHaveBeenCalled();
    await f.controller.act("clearStorageCache");
    expect(f.port.clear).toHaveBeenCalledTimes(1);
    await f.controller.act("resumeStorage");
    expect(f.resume).toHaveBeenCalledTimes(1);
  });
  it("a repeated disk-full failure waits for another user gesture", async () => {
    const f = fixture();
    await f.controller.open("batch", "Libere espaço.");
    await f.controller.act("clearStorageCache");
    await f.controller.open("batch", "Ainda sem espaço.");
    expect(f.resume).toHaveBeenCalledTimes(1);
    expect(f.port.clear).toHaveBeenCalledTimes(1);
    expect(f.states[f.states.length - 1]?.busy).toBe(false);
  });
  it("cannot resume an obsolete dialog after its cleanup finishes", async () => {
    let finish!: (freed: boolean) => void;
    const f = fixture(vi.fn(() => new Promise<boolean>(resolve => { finish = resolve; })));
    await f.controller.open("batch", "Libere espaço.");
    const pending = f.controller.act("clearStorageCache");
    f.controller.dispose();
    finish(true); await pending;
    expect(f.resume).not.toHaveBeenCalled();
  });
});
