import { expect, test } from "vitest";

import {
  matchProjectCommandShortcut,
  projectCommandShortcutLabel,
} from "./projectCommandCatalog";
import { createProjectApplicationMenus } from "./projectApplicationMenus";

function keyboardShortcut(
  key: string,
  modifiers: Partial<
    Pick<KeyboardEvent, "altKey" | "ctrlKey" | "metaKey" | "shiftKey">
  > = {},
) {
  return {
    altKey: false,
    ctrlKey: false,
    key,
    metaKey: false,
    shiftKey: false,
    ...modifiers,
  };
}

test("keeps displayed Project command shortcuts and accepted aliases in one catalog", () => {
  expect(projectCommandShortcutLabel("save")).toBe("Ctrl+S");
  expect(projectCommandShortcutLabel("save-as")).toBe("Ctrl+Shift+S");
  expect(projectCommandShortcutLabel("close")).toBe("Ctrl+W");
  expect(projectCommandShortcutLabel("undo")).toBe("Ctrl+Z");
  expect(projectCommandShortcutLabel("redo")).toBe("Ctrl+Shift+Z");

  expect(
    matchProjectCommandShortcut(
      keyboardShortcut("Z", { ctrlKey: true, shiftKey: true }),
    ),
  ).toBe("redo");
  expect(
    matchProjectCommandShortcut(keyboardShortcut("y", { ctrlKey: true })),
  ).toBe("redo");
  expect(
    matchProjectCommandShortcut(
      keyboardShortcut("s", { ctrlKey: true, shiftKey: true }),
    ),
  ).toBe("save-as");
});

test("feeds the canonical shortcuts into the Project application menu", () => {
  const groups = createProjectApplicationMenus({
    canExport: true,
    canRedo: true,
    canUndo: true,
    closeProject: () => undefined,
    exportSheet: () => undefined,
    redo: () => undefined,
    save: () => undefined,
    undo: () => undefined,
  });
  const commands = groups.flatMap((group) =>
    group.items.flatMap((item) => {
      if (item.type === "command") return [item];
      if (item.type === "submenu") return item.items;
      return [];
    }),
  );
  const displayedShortcut = (commandId: string) =>
    commands.find((command) => command.id === commandId)?.shortcut;

  expect(displayedShortcut("save")).toBe("Ctrl+S");
  expect(displayedShortcut("save-as")).toBe("Ctrl+Shift+S");
  expect(displayedShortcut("close")).toBe("Ctrl+W");
  expect(displayedShortcut("undo")).toBe("Ctrl+Z");
  expect(displayedShortcut("redo")).toBe("Ctrl+Shift+Z");
});
