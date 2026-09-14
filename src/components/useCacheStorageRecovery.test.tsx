import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { ProjectDialogAction, ProjectDialogPort } from "../application/projectDialogPort";
import type { ImageProcessingProgress, MediaPreviewPort } from "../application/projectPorts";
import { useCacheStorageRecovery } from "./useCacheStorageRecovery";

test("a delayed initial status cannot invalidate cleanup opened by a warning", async () => {
  let initialStatus!: (value: { id: string; canClearCache: boolean }) => void;
  let finishCleanup!: (value: boolean) => void;
  let action!: (action: ProjectDialogAction) => void;
  const paused = { id: "paused", canClearCache: true };
  const present = vi.fn(async () => {});
  const port = {
    storageRecovery: {
      status: vi.fn().mockImplementationOnce(() => new Promise(resolve => { initialStatus = resolve; }))
        .mockResolvedValue(paused),
      clear: vi.fn(() => new Promise<boolean>(resolve => { finishCleanup = resolve; })),
    },
    resumeCacheImages: vi.fn(async () => true),
  } as unknown as MediaPreviewPort;
  const dialogPort: ProjectDialogPort = { acquire: listener => {
    action = listener; return { present, dismiss: vi.fn(async () => {}) };
  } };
  const hook = renderHook(({ warning }) => useCacheStorageRecovery({ projectId: "album", enabled: true,
    warning, port, dialogPort, onResumed: vi.fn() }), { initialProps: { warning: false } });
  hook.rerender({ warning: true });
  await waitFor(() => expect(present).toHaveBeenLastCalledWith(expect.objectContaining({ canClearCache: true })));
  await act(async () => action("clearStorageCache"));
  await act(async () => initialStatus(paused));
  expect(present).toHaveBeenLastCalledWith(expect.objectContaining({ busy: true }));
  await act(async () => finishCleanup(true));
  expect(port.resumeCacheImages).toHaveBeenCalledOnce();
});

test("cache recovery awaits cleanup, uses determinate progress and waits again on disk exhaustion", async () => {
  let action!: (action: ProjectDialogAction) => void;
  let finishCleanup!: (freed: boolean) => void;
  let finishImages!: (completed: boolean) => void;
  let progress!: (progress: ImageProcessingProgress) => void;
  const present = vi.fn(async () => {}), dismiss = vi.fn(async () => {});
  const dialogPort: ProjectDialogPort = { acquire: listener => { action = listener; return { present, dismiss }; } };
  const port = { storageRecovery: { status: vi.fn(async () => ({ id: "paused", canClearCache: true })),
    clear: vi.fn(() => new Promise<boolean>(resolve => { finishCleanup = resolve; })) },
    resumeCacheImages: vi.fn((callback: typeof progress) => { progress = callback; return new Promise<boolean>(resolve => { finishImages = resolve; }); }),
  } as unknown as MediaPreviewPort;
  const onResumed = vi.fn();
  const hook = renderHook(({ enabled }) => useCacheStorageRecovery({ projectId: "album", enabled, warning: false, port, dialogPort, onResumed }), { initialProps: { enabled: false } });
  expect(present).not.toHaveBeenCalled();
  hook.rerender({ enabled: true });
  await waitFor(() => expect(present).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "storageFull", canClearCache: true })));
  await act(async () => action("clearStorageCache"));
  expect(port.resumeCacheImages).not.toHaveBeenCalled();
  await act(async () => finishCleanup(true));
  expect(port.resumeCacheImages).toHaveBeenCalledOnce();
  await act(async () => progress({ completedFiles: 3, totalFiles: 10, problem: null }));
  expect(present).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "imageProcessingProgress", progress: expect.objectContaining({ kind: "determinate", completed: 3, total: 10 }) }));
  await act(async () => finishImages(false));
  expect(present).toHaveBeenLastCalledWith(expect.objectContaining({ kind: "storageFull", busy: false }));
  expect(port.resumeCacheImages).toHaveBeenCalledOnce();
  await act(async () => action("cancelStorage"));
  const count = present.mock.calls.length;
  hook.rerender({ enabled: false }); hook.rerender({ enabled: true });
  expect(present).toHaveBeenCalledTimes(count);
  expect(onResumed).not.toHaveBeenCalled();
});
