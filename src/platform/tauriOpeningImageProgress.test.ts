// @vitest-environment node
import { beforeEach, expect, test, vi } from "vitest";

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

test("reports nothing while the host has no progress snapshot yet", async () => {
  let emit!: (payload: unknown) => void;
  api.listen.mockImplementation(async (_event: string, handler: (event: { payload: unknown }) => void) => {
    emit = (payload) => handler({ payload });
    return api.unlisten;
  });
  api.invoke.mockResolvedValue(null);
  const receive = vi.fn();

  const subscription = subscribeOpeningImageProgress(receive);
  await subscription.ready;
  expect(api.invoke).toHaveBeenCalledExactlyOnceWith("opening_image_progress");
  expect(receive).not.toHaveBeenCalled();

  emit(null);
  expect(receive).not.toHaveBeenCalled();

  emit({ completedFiles: 1, totalFiles: 12 });
  expect(receive).toHaveBeenCalledExactlyOnceWith({ completedFiles: 1, totalFiles: 12 });
  subscription.dispose();
});
