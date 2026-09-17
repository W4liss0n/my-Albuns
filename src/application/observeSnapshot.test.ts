import { expect, test, vi } from "vitest";
import { observeSnapshot } from "./observeSnapshot";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

test("waits for registration and gives live events precedence over a late snapshot", async () => {
  const registration = deferred<() => void>();
  const snapshot = deferred<number | null>();
  const receive = vi.fn();
  const read = vi.fn(() => snapshot.promise);
  let emit!: (value: number) => void;
  const observation = observeSnapshot({
    subscribe: listener => { emit = listener; return registration.promise; }, read, receive,
  });
  expect(read).not.toHaveBeenCalled();
  emit(7);
  registration.resolve(vi.fn());
  await Promise.resolve();
  expect(read).toHaveBeenCalledOnce();
  snapshot.resolve(2);
  await observation.ready;
  emit(8);
  expect(receive.mock.calls).toEqual([[7], [8]]);
  observation.dispose();
});

test.each([3, null])("delivers initial state %s when no live event arrived", async value => {
  const receive = vi.fn();
  const stop = vi.fn();
  const observation = observeSnapshot({ subscribe: async () => stop, read: async () => value, receive });
  await observation.ready;
  expect(receive).toHaveBeenCalledWith(value);
  observation.dispose();
  observation.dispose();
  expect(stop).toHaveBeenCalledOnce();
});

test("disposal before registration completes releases the late listener without reading", async () => {
  const registration = deferred<() => void>();
  const read = vi.fn(async () => 1);
  const receive = vi.fn();
  const stop = vi.fn();
  let emit!: (value: number) => void;
  const observation = observeSnapshot({ subscribe: listener => { emit = listener; return registration.promise; }, read, receive });
  observation.dispose();
  emit(3);
  registration.resolve(stop);
  await observation.ready;
  expect(stop).toHaveBeenCalledOnce();
  expect(read).not.toHaveBeenCalled();
  expect(receive).not.toHaveBeenCalled();
});

test.each([false, true])("disposal suppresses a pending snapshot including failure=%s", async fails => {
  const snapshot = deferred<number | null>();
  const receive = vi.fn();
  const stop = vi.fn();
  const observation = observeSnapshot({ subscribe: async () => stop, read: () => snapshot.promise, receive });
  await Promise.resolve();
  observation.dispose();
  if (fails) snapshot.reject(new Error("closed")); else snapshot.resolve(2);
  await observation.ready;
  expect(stop).toHaveBeenCalledOnce();
  expect(receive).not.toHaveBeenCalled();
});

test("snapshot failure releases the installed subscription and reports the error", async () => {
  const stop = vi.fn();
  const observation = observeSnapshot({ subscribe: async () => stop, read: async () => { throw new Error("snapshot"); }, receive: vi.fn() });
  await expect(observation.ready).rejects.toThrow("snapshot");
  observation.dispose();
  expect(stop).toHaveBeenCalledOnce();
});

test("registration failure never starts the snapshot", async () => {
  const read = vi.fn(async () => 1);
  const observation = observeSnapshot({ subscribe: async () => { throw new Error("registration"); }, read, receive: vi.fn() });
  await expect(observation.ready).rejects.toThrow("registration");
  expect(read).not.toHaveBeenCalled();
});
