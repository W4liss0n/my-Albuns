// @vitest-environment node
import { beforeEach, expect, test, vi } from "vitest";

const api = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn(), unlisten: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: api.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: api.listen }));
import { parseOpeningProgress, subscribeOpeningProgress } from "./tauriOpeningProgress";

const preparing = {
  name: "SARAH XAVIER", state: "preparing", completedFiles: 32, totalFiles: 70,
};

beforeEach(() => {
  vi.resetAllMocks();
  api.listen.mockResolvedValue(api.unlisten);
});

test("recovers the current rows after installing the live subscription", async () => {
  api.invoke.mockResolvedValue({ projects: [preparing] });
  const receive = vi.fn();
  const subscription = subscribeOpeningProgress(receive);
  await subscription.ready;
  expect(api.listen.mock.invocationCallOrder[0]).toBeLessThan(api.invoke.mock.invocationCallOrder[0]);
  expect(api.listen).toHaveBeenCalledWith("myalbuns://opening-progress", expect.any(Function), {
    target: "dialog-opening-progress",
  });
  expect(api.invoke).toHaveBeenCalledExactlyOnceWith("opening_progress");
  expect(receive).toHaveBeenCalledWith({ projects: [preparing] });
  subscription.dispose();
  expect(api.unlisten).toHaveBeenCalledOnce();
});

test("reports nothing while no rows are available, including during a decision", async () => {
  let emit!: (payload: unknown) => void;
  api.listen.mockImplementation(async (_event: string, handler: (event: { payload: unknown }) => void) => {
    emit = (payload) => handler({ payload });
    return api.unlisten;
  });
  api.invoke.mockResolvedValue(null);
  const receive = vi.fn();

  const subscription = subscribeOpeningProgress(receive);
  await subscription.ready;
  expect(receive).not.toHaveBeenCalled();

  emit(null);
  emit({ projects: [{ ...preparing, state: "unknown" }] });
  emit({ projects: [{ ...preparing, totalFiles: -1 }] });
  expect(receive).not.toHaveBeenCalled();

  emit({ projects: [preparing, { name: "YUELSON RODRIGO", state: "ready", completedFiles: 0, totalFiles: 0 }] });
  expect(receive).toHaveBeenCalledExactlyOnceWith({
    projects: [preparing, { name: "YUELSON RODRIGO", state: "ready", completedFiles: 0, totalFiles: 0 }],
  });
  subscription.dispose();
});

test("never carries more than the rows the window shows", () => {
  expect(parseOpeningProgress({
    projects: [{ ...preparing, path: "\\servidor\clientes\SARAH XAVIER.myalbuns" }],
  })).toEqual({ projects: [preparing] });
});
