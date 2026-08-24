export interface ProjectCommandShortcutEvent {
  altKey: boolean;
  ctrlKey: boolean;
  key: string;
  metaKey: boolean;
  shiftKey: boolean;
}

interface ProjectCommandShortcut {
  altKey?: boolean;
  ctrlKey?: boolean;
  display?: string;
  key: string;
  shiftKey?: boolean;
}

interface ProjectCommandDefinition {
  id: string;
  shortcuts: readonly ProjectCommandShortcut[];
}

const PROJECT_COMMAND_CATALOG = [
  command("new-project", shortcut("n", "Ctrl+N", { ctrlKey: true })),
  command("open-project", shortcut("o", "Ctrl+O", { ctrlKey: true })),
  command("save", shortcut("s", "Ctrl+S", { ctrlKey: true })),
  command(
    "save-as",
    shortcut("s", "Ctrl+Shift+S", { ctrlKey: true, shiftKey: true }),
  ),
  command("close", shortcut("w", "Ctrl+W", { ctrlKey: true })),
  command("undo", shortcut("z", "Ctrl+Z", { ctrlKey: true })),
  command(
    "redo",
    shortcut("z", "Ctrl+Shift+Z", { ctrlKey: true, shiftKey: true }),
    shortcut("y", undefined, { ctrlKey: true }),
  ),
  command("copy-frames", shortcut("c", "Ctrl+C", { ctrlKey: true })),
  command("paste-frames", shortcut("v", "Ctrl+V", { ctrlKey: true })),
  command(
    "advance-frames",
    shortcut("]", "Ctrl+]", { ctrlKey: true }),
  ),
  command(
    "recede-frames",
    shortcut("[", "Ctrl+[", { ctrlKey: true }),
  ),
  command("select-all", shortcut("a", "Ctrl+A", { ctrlKey: true })),
  command("delete-sheet", shortcut("delete", "Delete")),
] as const satisfies readonly ProjectCommandDefinition[];

export type ProjectCommandId =
  (typeof PROJECT_COMMAND_CATALOG)[number]["id"];

export function projectCommandShortcutLabel(
  commandId: ProjectCommandId,
) {
  const definition = PROJECT_COMMAND_CATALOG.find(
    (candidate) => candidate.id === commandId,
  );
  return definition?.shortcuts.find(
    (candidate) => candidate.display !== undefined,
  )?.display;
}

export function matchProjectCommandShortcut(
  event: ProjectCommandShortcutEvent,
): ProjectCommandId | null {
  const eventKey = event.key.toLowerCase();
  for (const definition of PROJECT_COMMAND_CATALOG) {
    const matched = definition.shortcuts.some(
      (candidate) =>
        candidate.key === eventKey &&
        Boolean(candidate.altKey) === event.altKey &&
        Boolean(candidate.ctrlKey) === event.ctrlKey &&
        Boolean(candidate.shiftKey) === event.shiftKey &&
        !event.metaKey,
    );
    if (matched) return definition.id;
  }
  return null;
}

function command<const Id extends string>(
  id: Id,
  ...shortcuts: readonly ProjectCommandShortcut[]
) {
  return { id, shortcuts };
}

function shortcut(
  key: string,
  display?: string,
  modifiers: Pick<
    ProjectCommandShortcut,
    "altKey" | "ctrlKey" | "shiftKey"
  > = {},
): ProjectCommandShortcut {
  return { display, key, ...modifiers };
}
