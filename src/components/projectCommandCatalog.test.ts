import { expect, test } from "vitest";

import {
  PROJECT_COMMAND_CATALOG,
  matchProjectCommandShortcut,
  projectCommandBinding,
  projectCommandShortcutAria,
  projectCommandShortcutLabel,
} from "../application/projectCommandCatalog";
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
      "project-window",
    ),
  ).toBe("redo");
  expect(
    matchProjectCommandShortcut(
      keyboardShortcut("y", { ctrlKey: true }),
      "project-window",
    ),
  ).toBe("redo");
  expect(
    matchProjectCommandShortcut(
      keyboardShortcut("s", { ctrlKey: true, shiftKey: true }),
      "project-window",
    ),
  ).toBe("save-as");
});

test("feeds the canonical shortcuts into the Project application menu", () => {
  const groups = createProjectApplicationMenus({
    canExport: true,
    canRedo: true,
    canUndo: true,
    contextualPanelVisible: true,
    closeProject: () => undefined,
    exportSheet: () => undefined,
    mediaPanelVisible: true,
    redo: () => undefined,
    save: () => undefined,
    saveAs: () => undefined,
    undo: () => undefined,
    toggleContextualPanel: () => undefined,
    toggleMediaPanel: () => undefined,
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

test("keeps stable command metadata complete and conflict-free by context", () => {
  const ids = new Set<string>();
  const associations = new Set<string>();

  for (const command of PROJECT_COMMAND_CATALOG) {
    expect(command.id).not.toBe("");
    expect(command.label).not.toBe("");
    expect(command.description).not.toBe("");
    expect(command.bindings.length).toBeGreaterThan(0);
    expect(ids.has(command.id), command.id).toBe(false);
    ids.add(command.id);

    for (const binding of command.bindings) {
      for (const shortcut of command.shortcuts) {
        const association = [
          binding.context,
          shortcut.ctrlKey ? "Ctrl" : "",
          shortcut.altKey ? "Alt" : "",
          shortcut.shiftKey ? "Shift" : "",
          shortcut.key.toLowerCase(),
        ].join("+");
        expect(associations.has(association), association).toBe(false);
        associations.add(association);
      }
    }
  }
});

test("registers contextual Ctrl+E without pretending Photoshop is implemented", () => {
  const photoshop = PROJECT_COMMAND_CATALOG.find(
    (command) => command.id === "open-in-photoshop",
  );

  expect(photoshop).toMatchObject({
    label: "Abrir no Photoshop",
  });
  expect(projectCommandBinding("open-in-photoshop", "frame-photo"))
    .toMatchObject({ availability: "placeholder" });
  expect(projectCommandBinding("open-in-photoshop", "media-photo"))
    .toMatchObject({ availability: "placeholder" });
  expect(projectCommandShortcutLabel("open-in-photoshop")).toBe("Ctrl+E");
  expect(
    matchProjectCommandShortcut(
      keyboardShortcut("e", { ctrlKey: true }),
      "frame-photo",
    ),
  ).toBe("open-in-photoshop");
});

test("shares New and Open metadata with the Welcome surface without borrowing the Project context", () => {
  expect(projectCommandBinding("new-project", "welcome")).toEqual({
    availability: "implemented",
    context: "welcome",
  });
  expect(projectCommandBinding("open-project", "welcome")).toEqual({
    availability: "implemented",
    context: "welcome",
  });
  expect(projectCommandBinding("new-project", "project-window")).toMatchObject({
    availability: "placeholder",
  });

  expect(projectCommandShortcutLabel("new-project")).toBe("Ctrl+N");
  expect(projectCommandShortcutLabel("open-project")).toBe("Ctrl+O");
  expect(projectCommandShortcutAria("new-project")).toBe("Control+N");
  expect(projectCommandShortcutAria("open-project")).toBe("Control+O");
  expect(
    matchProjectCommandShortcut(
      keyboardShortcut("n", { ctrlKey: true }),
      "welcome",
    ),
  ).toBe("new-project");
  expect(
    matchProjectCommandShortcut(
      keyboardShortcut("o", { ctrlKey: true }),
      "welcome",
    ),
  ).toBe("open-project");
});

test("represents Select all availability per owning context", () => {
  expect(projectCommandBinding("select-all", "media-panel")).toEqual({
    availability: "implemented",
    context: "media-panel",
  });
  expect(projectCommandBinding("select-all", "frame")).toEqual({
    availability: "placeholder",
    context: "frame",
    placeholderFeature: "select-all-in-active-context",
  });
});

test("keeps both Project panel visibility commands canonical and implemented", () => {
  expect(projectCommandBinding("media-panel", "project-window")).toEqual({
    availability: "implemented",
    context: "project-window",
  });
  expect(projectCommandBinding("contextual-panel", "project-window")).toEqual({
    availability: "implemented",
    context: "project-window",
  });
});

test("projects every application-menu command from its canonical descriptor", () => {
  const descriptors = new Map(
    PROJECT_COMMAND_CATALOG.map(
      (command) => [command.id as string, command] as const,
    ),
  );
  const groups = createProjectApplicationMenus({
    canExport: true,
    canRedo: true,
    canUndo: true,
    contextualPanelVisible: true,
    closeProject: () => undefined,
    exportSheet: () => undefined,
    mediaPanelVisible: true,
    redo: () => undefined,
    save: () => undefined,
    saveAs: () => undefined,
    undo: () => undefined,
    toggleContextualPanel: () => undefined,
    toggleMediaPanel: () => undefined,
  });
  const commands = groups.flatMap((group) =>
    group.items.flatMap((item) => {
      if (item.type === "command") return [item];
      if (item.type === "submenu") return item.items;
      return [];
    }),
  );

  for (const command of commands) {
    const descriptor = descriptors.get(command.id);
    const binding = descriptor?.bindings.find(
      (candidate) => candidate.context === command.context,
    );
    expect(descriptor, command.id).toBeDefined();
    expect(command.label).toBe(descriptor?.label);
    expect(command.shortcut).toBe(
      descriptor?.shortcuts.find((shortcut) => shortcut.display)?.display,
    );
    expect(command.availability).toBe(binding?.availability);
    if (command.availability === "placeholder") {
      expect(command.feature).toBe(
        binding?.availability === "placeholder"
          ? binding.placeholderFeature
          : undefined,
      );
    }
  }
});
