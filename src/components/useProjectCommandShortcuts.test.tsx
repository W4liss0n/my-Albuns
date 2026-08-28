import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { useProjectCommandShortcuts } from "./useProjectCommandShortcuts";

function handlers() {
  return {
    canDeleteSheet: true,
    closeProject: vi.fn(),
    deleteSheet: vi.fn(),
    redo: vi.fn(),
    save: vi.fn(),
    saveAs: vi.fn(),
    sheetShortcutActive: true,
    sheetCommandsDisabled: false,
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
  expect(
    dispatchShortcut("s", { shiftKey: true }).defaultPrevented,
  ).toBe(true);
  expect(dispatchShortcut("z").defaultPrevented).toBe(true);
  expect(
    dispatchShortcut("z", { shiftKey: true }).defaultPrevented,
  ).toBe(true);
  expect(dispatchShortcut("y").defaultPrevented).toBe(true);
  expect(dispatchShortcut("w").defaultPrevented).toBe(true);

  expect(actions.save).toHaveBeenCalledOnce();
  expect(actions.saveAs).toHaveBeenCalledOnce();
  expect(actions.undo).toHaveBeenCalledOnce();
  expect(actions.redo).toHaveBeenCalledTimes(2);
  expect(actions.closeProject).toHaveBeenCalledOnce();
});

test("dispatches Save as distinctly from Save", () => {
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
  expect(actions.saveAs).toHaveBeenCalledOnce();
});

test("dispatches Delete only for an available Sheet outside text entry and Edit Mode", () => {
  const actions = handlers();
  const view = renderHook(
    ({ canDeleteSheet, sheetCommandsDisabled }) =>
      useProjectCommandShortcuts({
        ...actions,
        canDeleteSheet,
        canRedo: true,
        canUndo: true,
        disabled: false,
        sheetCommandsDisabled,
      }),
    {
      initialProps: {
        canDeleteSheet: true,
        sheetCommandsDisabled: false,
      },
    },
  );

  expect(dispatchShortcut("Delete", { ctrlKey: false }).defaultPrevented).toBe(
    true,
  );
  expect(actions.deleteSheet).toHaveBeenCalledOnce();

  const input = document.createElement("input");
  document.body.append(input);
  try {
    expect(
      dispatchShortcut("Delete", { ctrlKey: false }, input).defaultPrevented,
    ).toBe(false);
    expect(actions.deleteSheet).toHaveBeenCalledOnce();
  } finally {
    input.remove();
  }

  view.rerender({ canDeleteSheet: false, sheetCommandsDisabled: false });
  expect(dispatchShortcut("Delete", { ctrlKey: false }).defaultPrevented).toBe(
    true,
  );
  expect(actions.deleteSheet).toHaveBeenCalledOnce();

  view.rerender({ canDeleteSheet: true, sheetCommandsDisabled: true });
  expect(dispatchShortcut("Delete", { ctrlKey: false }).defaultPrevented).toBe(
    true,
  );
  expect(actions.deleteSheet).toHaveBeenCalledOnce();
});

test("leaves Delete to the Media Panel while a media item owns keyboard focus", () => {
  const actions = handlers();
  renderHook(() =>
    useProjectCommandShortcuts({
      ...actions,
      canRedo: true,
      canUndo: true,
      disabled: false,
    }),
  );
  const mediaPanel = document.createElement("section");
  mediaPanel.dataset.projectCommandContext = "media-panel";
  const media = document.createElement("button");
  mediaPanel.append(media);
  document.body.append(mediaPanel);

  try {
    media.focus();
    const event = dispatchShortcut("Delete", { ctrlKey: false }, media);

    expect(event.defaultPrevented).toBe(false);
    expect(actions.deleteSheet).not.toHaveBeenCalled();
  } finally {
    mediaPanel.remove();
  }
});

test("leaves Delete to the selected Frame when the Sheet context is inactive", () => {
  const actions = handlers();
  renderHook(() =>
    useProjectCommandShortcuts({
      ...actions,
      canRedo: true,
      canUndo: true,
      disabled: false,
      sheetShortcutActive: false,
    }),
  );

  const event = dispatchShortcut("Delete", { ctrlKey: false });

  expect(event.defaultPrevented).toBe(false);
  expect(actions.deleteSheet).not.toHaveBeenCalled();
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
