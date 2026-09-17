import { beforeEach, expect, test, vi } from "vitest";
import type { StartupImageProgress } from "../contracts/generated/StartupImageProgress";

const api = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), unlisten: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: api.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: api.listen }));
import { subscribeOpeningImageProgress } from "./tauriOpeningImageProgress";

beforeEach(() => {
  vi.resetAllMocks();
  api.listen.mockResolvedValue(api.unlisten);
});

test("recovers the current progress after installing the live subscription", async () => {
  api.invoke.mockResolvedValue({ completedFiles: 3, totalFiles: 12 });
  const receive = vi.fn();
  const subscription = subscribeOpeningImageProgress(receive);
  await subscription.ready;
  expect(api.listen.mock.invocationCallOrder[0]).toBeLessThan(api.invoke.mock.invocationCallOrder[0]);
  expect(api.listen).toHaveBeenCalledWith("myalbuns://opening-image-progress", expect.any(Function), {
    target: "dialog-opening-progress",
  });
  expect(receive).toHaveBeenCalledWith({ completedFiles: 3, totalFiles: 12 });
  subscription.dispose();
  expect(api.unlisten).toHaveBeenCalledOnce();
});

test("a late snapshot cannot move live progress backwards", async () => {
  let completeSnapshot!: (progress: StartupImageProgress) => void;
  api.invoke.mockImplementation(() => new Promise(resolve => { completeSnapshot = resolve; }));
  const receive = vi.fn();
  const subscription = subscribeOpeningImageProgress(receive);
  await vi.waitFor(() => expect(api.invoke).toHaveBeenCalled());
  api.listen.mock.calls[0][1]({ payload: { completedFiles: 7, totalFiles: 12 } });
  completeSnapshot({ completedFiles: 2, totalFiles: 12 });
  await subscription.ready;
  expect(receive.mock.calls).toEqual([[{ completedFiles: 7, totalFiles: 12 }]]);
});

test("releases the subscription when the snapshot command fails", async () => {
  api.invoke.mockRejectedValue(new Error("closed"));
  await expect(subscribeOpeningImageProgress(vi.fn()).ready).rejects.toThrow("closed");
  expect(api.unlisten).toHaveBeenCalledOnce();
});

test("disposal before registration skips the snapshot and releases the late listener", async () => {
  let registered!: (stop: () => void) => void;
  api.listen.mockImplementation(() => new Promise(resolve => { registered = resolve; }));
  const receive = vi.fn();
  const subscription = subscribeOpeningImageProgress(receive);
  subscription.dispose();
  registered(api.unlisten);
  await subscription.ready;
  api.listen.mock.calls[0][1]({ payload: { completedFiles: 3, totalFiles: 12 } });
  expect(api.invoke).not.toHaveBeenCalled();
  expect(receive).not.toHaveBeenCalled();
  expect(api.unlisten).toHaveBeenCalledOnce();
});

test("disposal releases live delivery while the initial snapshot is pending", async () => {
  let completeSnapshot!: (progress: StartupImageProgress) => void;
  api.invoke.mockImplementation(() => new Promise(resolve => { completeSnapshot = resolve; }));
  const receive = vi.fn();
  const subscription = subscribeOpeningImageProgress(receive);
  await vi.waitFor(() => expect(api.invoke).toHaveBeenCalled());
  subscription.dispose();
  expect(api.unlisten).toHaveBeenCalledOnce();
  completeSnapshot({ completedFiles: 2, totalFiles: 12 });
  await subscription.ready;
  expect(receive).not.toHaveBeenCalled();
});
