import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { useProjectCommandShortcuts } from "./useProjectCommandShortcuts";

function handlers() {
  return {
    closeProject: vi.fn(),
    redo: vi.fn(),
    save: vi.fn(),
    undo: vi.fn(),
  };
}

function dispatchShortcut(
  key: string,
  options: KeyboardEventInit = {},
  target: HTMLElement | Window = window,
) {
  const event = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    ctrlKey: true,
    key,
    ...options,
  });
  act(() => {
    target.dispatchEvent(event);
  });
  return event;
}

test("dispatches the implemented Project command for every accepted shortcut", () => {
  const actions = handlers();
  renderHook(() =>
    useProjectCommandShortcuts({
      ...actions,
      canRedo: true,
      canUndo: true,
      disabled: false,
    }),
  );

  expect(dispatchShortcut("s").defaultPrevented).toBe(true);
  expect(dispatchShortcut("z").defaultPrevented).toBe(true);
  expect(
    dispatchShortcut("z", { shiftKey: true }).defaultPrevented,
  ).toBe(true);
  expect(dispatchShortcut("y").defaultPrevented).toBe(true);
  expect(dispatchShortcut("w").defaultPrevented).toBe(true);

  expect(actions.save).toHaveBeenCalledOnce();
  expect(actions.undo).toHaveBeenCalledOnce();
  expect(actions.redo).toHaveBeenCalledTimes(2);
  expect(actions.closeProject).toHaveBeenCalledOnce();
});

test("keeps placeholder Save as distinct from Save", () => {
  const actions = handlers();
  renderHook(() =>
    useProjectCommandShortcuts({
      ...actions,
      canRedo: true,
      canUndo: true,
      disabled: false,
    }),
  );

  const event = dispatchShortcut("s", { shiftKey: true });

  expect(event.defaultPrevented).toBe(true);
  expect(actions.save).not.toHaveBeenCalled();
});

test("leaves contextual Ctrl+E to the active photo owner", () => {
  const actions = handlers();
  renderHook(() =>
    useProjectCommandShortcuts({
      ...actions,
      canRedo: true,
      canUndo: true,
      disabled: false,
    }),
  );

  const event = dispatchShortcut("e");

  expect(event.defaultPrevented).toBe(false);
  expect(actions.save).not.toHaveBeenCalled();
  expect(actions.closeProject).not.toHaveBeenCalled();
});

test("leaves text Undo and Redo to editable targets", () => {
  const actions = handlers();
  renderHook(() =>
    useProjectCommandShortcuts({
      ...actions,
      canRedo: true,
      canUndo: true,
      disabled: false,
    }),
  );
  const contentEditable = document.createElement("div");
  contentEditable.setAttribute("contenteditable", "true");
  const contentEditableChild = document.createElement("span");
  contentEditable.append(contentEditableChild);
  const targets = [
    document.createElement("input"),
    document.createElement("textarea"),
    document.createElement("select"),
    contentEditable,
    contentEditableChild,
  ];
  for (const target of targets) {
    if (!target.isConnected) document.body.append(target);
  }

  try {
    for (const target of targets) {
      expect(
        dispatchShortcut("z", {}, target).defaultPrevented,
        target.outerHTML,
      ).toBe(false);
      expect(
        dispatchShortcut("y", {}, target).defaultPrevented,
        target.outerHTML,
      ).toBe(false);
      expect(
        dispatchShortcut("z", { shiftKey: true }, target).defaultPrevented,
        target.outerHTML,
      ).toBe(false);
    }

    expect(actions.undo).not.toHaveBeenCalled();
    expect(actions.redo).not.toHaveBeenCalled();
  } finally {
    targets.forEach((target) => target.remove());
  }
});

test("consumes key repeats without repeating Project mutations", () => {
  const actions = handlers();
  renderHook(() =>
    useProjectCommandShortcuts({
      ...actions,
      canRedo: true,
      canUndo: true,
      disabled: false,
    }),
  );

  const event = dispatchShortcut("s", { repeat: true });

  expect(event.defaultPrevented).toBe(true);
  expect(actions.save).not.toHaveBeenCalled();
});

test("keeps blocked and unavailable Project commands inactive", () => {
  const blockedActions = handlers();
  const view = renderHook(
    ({ disabled }) =>
      useProjectCommandShortcuts({
        ...blockedActions,
        canRedo: false,
        canUndo: false,
        disabled,
      }),
    { initialProps: { disabled: true } },
  );

  expect(dispatchShortcut("s").defaultPrevented).toBe(true);
  expect(dispatchShortcut("w").defaultPrevented).toBe(true);
  expect(dispatchShortcut("z").defaultPrevented).toBe(true);
  expect(dispatchShortcut("y").defaultPrevented).toBe(true);
  expect(blockedActions.save).not.toHaveBeenCalled();
  expect(blockedActions.closeProject).not.toHaveBeenCalled();
  expect(blockedActions.undo).not.toHaveBeenCalled();
  expect(blockedActions.redo).not.toHaveBeenCalled();

  view.rerender({ disabled: false });
  dispatchShortcut("z");
  dispatchShortcut("y");
  expect(blockedActions.undo).not.toHaveBeenCalled();
  expect(blockedActions.redo).not.toHaveBeenCalled();
});
