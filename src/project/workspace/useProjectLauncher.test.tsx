import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { useProjectLauncher } from "./useProjectLauncher";

test("serializes menu and shortcut requests without queuing extra dialogs and permits retry after cancellation", async () => {
  let resolve!: () => void;
  const port = { openProject: vi.fn(() => new Promise<void>(r => { resolve = r; })), newProject: vi.fn(async () => undefined) };
  const error = vi.fn();
  const { result } = renderHook(() => useProjectLauncher(port, false, error));
  act(() => { result.current.openProject?.(); result.current.openProject?.(); result.current.newProject?.(); });
  expect(port.openProject).toHaveBeenCalledTimes(1);
  expect(port.newProject).not.toHaveBeenCalled();
  await act(async () => resolve());
  await act(async () => result.current.newProject?.());
  expect(port.newProject).toHaveBeenCalledOnce();
  expect(error).not.toHaveBeenCalled();
});

test("reports failures through the existing result dialog and releases admission for retry", async () => {
  const port = { openProject: vi.fn().mockRejectedValueOnce("Não foi possível abrir").mockResolvedValue(undefined), newProject: vi.fn() };
  const error = vi.fn();
  const { result, rerender } = renderHook(({ disabled }) => useProjectLauncher(port, disabled, error), { initialProps: { disabled: false } });
  await act(async () => result.current.openProject?.());
  expect(error).toHaveBeenCalledExactlyOnceWith("Não foi possível abrir");
  await act(async () => result.current.openProject?.());
  expect(port.openProject).toHaveBeenCalledTimes(2);
  rerender({ disabled: true });
  expect(result.current.newProject).toBeUndefined();
  expect(result.current.openProject).toBeUndefined();
});
