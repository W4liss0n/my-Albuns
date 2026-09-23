import { beforeEach, expect, test, vi } from "vitest";
import type { Face } from "./faceLandmarks";

class FakeWorker {
  static instances: FakeWorker[] = [];
  messages: Array<{ id?: number; cancel?: number }> = [];
  onmessage: ((event: MessageEvent<{ id: number; faces?: Face[]; error?: string }>) => void) | null = null;
  onerror: (() => void) | null = null;
  terminate = vi.fn();
  constructor() { FakeWorker.instances.push(this); }
  postMessage(message: { id?: number; cancel?: number }) { this.messages.push(message); }
  respond(id: number, faces: Face[]) { this.onmessage?.({ data: { id, faces } } as MessageEvent); }
}

const image = (src: string) => ({ src, currentSrc: src, naturalWidth: 800, naturalHeight: 600 }) as HTMLImageElement;
const face: Face = [{ x: .3, y: .3, z: 0 }, { x: .6, y: .6, z: 0 }];

beforeEach(() => {
  vi.resetModules();
  FakeWorker.instances = [];
  vi.stubGlobal("Worker", FakeWorker);
  vi.stubGlobal("createImageBitmap", vi.fn(async () => ({ close: vi.fn() })));
});

test("shared detection survives one consumer release and caches only the completed result", async () => {
  const { acquireFaces } = await import("./faceLandmarks");
  const first = acquireFaces(image("shared"));
  const second = acquireFaces(image("shared"));
  await vi.waitFor(() => expect(FakeWorker.instances[0]?.messages).toHaveLength(1));
  const stopped = expect(first.promise).rejects.toHaveProperty("name", "AbortError");
  first.release();
  await stopped;
  expect(FakeWorker.instances[0].messages).toHaveLength(1);
  FakeWorker.instances[0].respond(FakeWorker.instances[0].messages[0].id!, [face]);
  await expect(second.promise).resolves.toEqual([face]);
  second.release();
  await expect(acquireFaces(image("shared")).promise).resolves.toEqual([face]);
  expect(FakeWorker.instances[0].messages).toHaveLength(1);
});

test("abandoned A is cancelled and a quick A revisit starts a fresh scan", async () => {
  const { acquireFaces } = await import("./faceLandmarks");
  const oldA = acquireFaces(image("A"));
  await vi.waitFor(() => expect(FakeWorker.instances[0]?.messages).toHaveLength(1));
  const stopped = expect(oldA.promise).rejects.toHaveProperty("name", "AbortError");
  oldA.release();
  await stopped;
  const B = acquireFaces(image("B"));
  const newA = acquireFaces(image("A"));
  await vi.waitFor(() => expect(FakeWorker.instances[0].messages).toHaveLength(4));
  const [first, cancel, second, third] = FakeWorker.instances[0].messages;
  expect(cancel).toEqual({ cancel: first.id });
  expect(third.id).not.toBe(first.id);
  FakeWorker.instances[0].respond(first.id!, [face]);
  FakeWorker.instances[0].respond(second.id!, [face]);
  FakeWorker.instances[0].respond(third.id!, [face]);
  await expect(B.promise).resolves.toEqual([face]);
  await expect(newA.promise).resolves.toEqual([face]);
});

test("worker failure settles interests and the next scan opens a fresh worker", async () => {
  const { acquireFaces } = await import("./faceLandmarks");
  const first = acquireFaces(image("broken"));
  await vi.waitFor(() => expect(FakeWorker.instances[0]?.messages).toHaveLength(1));
  const failed = expect(first.promise).rejects.toThrow("Não foi possível analisar os rostos.");
  FakeWorker.instances[0].onerror?.();
  await failed;
  expect(FakeWorker.instances[0].terminate).toHaveBeenCalledOnce();
  const retry = acquireFaces(image("broken"));
  await vi.waitFor(() => expect(FakeWorker.instances).toHaveLength(2));
  FakeWorker.instances[1].respond(FakeWorker.instances[1].messages[0].id!, [face]);
  await expect(retry.promise).resolves.toEqual([face]);
});

test("release while creating a bitmap closes it without posting a cancelled job", async () => {
  const bitmap = { close: vi.fn() };
  let finishBitmap!: (value: typeof bitmap) => void;
  vi.stubGlobal("createImageBitmap", vi.fn(() => new Promise<typeof bitmap>((resolve) => { finishBitmap = resolve; })));
  const { acquireFaces } = await import("./faceLandmarks");
  const lease = acquireFaces(image("pending-bitmap"));
  const stopped = expect(lease.promise).rejects.toHaveProperty("name", "AbortError");
  lease.release();
  await stopped;
  finishBitmap(bitmap);
  await vi.waitFor(() => expect(bitmap.close).toHaveBeenCalledOnce());
  expect(FakeWorker.instances).toHaveLength(0);
});

test("a reported scan error is not cached", async () => {
  const { acquireFaces } = await import("./faceLandmarks");
  const first = acquireFaces(image("error"));
  await vi.waitFor(() => expect(FakeWorker.instances[0]?.messages).toHaveLength(1));
  const id = FakeWorker.instances[0].messages[0].id!;
  const failed = expect(first.promise).rejects.toThrow("model failed");
  FakeWorker.instances[0].onmessage?.({ data: { id, error: "model failed" } } as MessageEvent);
  await failed;
  const retry = acquireFaces(image("error"));
  await vi.waitFor(() => expect(FakeWorker.instances[0].messages).toHaveLength(2));
  FakeWorker.instances[0].respond(FakeWorker.instances[0].messages[1].id!, [face]);
  await expect(retry.promise).resolves.toEqual([face]);
});
