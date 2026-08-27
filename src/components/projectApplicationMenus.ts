import {
  projectCommandBinding,
  projectCommandDescriptor,
  projectCommandShortcutLabel,
  type ProjectCommandContext,
  type ProjectCommandId,
} from "../application/projectCommandCatalog";
import type {
  ApplicationMenuCommand,
  ApplicationMenuGroup,
} from "./ApplicationMenuBar";

interface ProjectApplicationMenuOptions {
  canExport: boolean;
  canRedo: boolean;
  canUndo: boolean;
  contextualPanelVisible: boolean;
  closeProject(): void;
  exportSheet(): void;
  mediaPanelVisible: boolean;
  redo(): void;
  save(): void;
  saveAs(): void;
  undo(): void;
  toggleContextualPanel(): void;
  toggleMediaPanel(): void;
}

export function createProjectApplicationMenus({
  canExport,
  canRedo,
  canUndo,
  contextualPanelVisible,
  closeProject,
  exportSheet,
  mediaPanelVisible,
  redo,
  save,
  saveAs,
  undo,
  toggleContextualPanel,
  toggleMediaPanel,
}: ProjectApplicationMenuOptions): readonly ApplicationMenuGroup[] {
  // PLACEHOLDER UI: the canonical catalog marks commands whose application
  // port or Project intent does not exist yet. Menus only project that state.
  return [
    {
      id: "file",
      label: "Arquivo",
      items: [
        placeholder("new-project", "project-window"),
        placeholder("open-project", "project-window"),
        separator("file-project-separator"),
        implemented("save", "project-window", save),
        implemented("save-as", "project-window", saveAs),
        separator("file-export-separator"),
        implemented("export-sheet", "sheet", exportSheet, !canExport),
        placeholder("export", "project-window"),
        separator("file-close-separator"),
        implemented("close", "project-window", closeProject),
      ],
    },
    {
      id: "edit",
      label: "Editar",
      items: [
        implemented("undo", "project-window", undo, !canUndo),
        implemented("redo", "project-window", redo, !canRedo),
        separator("edit-clipboard-separator"),
        placeholder("copy-frames", "frame"),
        placeholder("paste-frames", "frame"),
        separator("edit-frame-separator"),
        placeholder("swap-frame-contents", "frame"),
        placeholder("add-frame", "frame"),
        submenu("arrange-frames", "Organizar", [
          placeholder("bring-frames-to-front", "frame"),
          placeholder("advance-frames", "frame"),
          placeholder("recede-frames", "frame"),
          placeholder("send-frames-to-back", "frame"),
        ]),
        separator("edit-layout-separator"),
        placeholder("save-frame-arrangement-as-layout", "frame"),
        placeholder("select-all", "frame"),
      ],
    },
    {
      id: "sheet",
      label: "Lâmina",
      items: [
        placeholder("add-before", "sheet"),
        placeholder("add-after", "sheet"),
        placeholder("duplicate-sheet", "sheet"),
        placeholder("delete-sheet", "sheet"),
        separator("sheet-edge-separator"),
        placeholder("convert-edge", "sheet"),
      ],
    },
    {
      id: "view",
      label: "Exibir",
      items: [
        implemented(
          "media-panel",
          "project-window",
          toggleMediaPanel,
          false,
          mediaPanelVisible,
        ),
        implemented(
          "contextual-panel",
          "project-window",
          toggleContextualPanel,
          false,
          contextualPanelVisible,
        ),
        separator("view-canvas-separator"),
        placeholder("fit-sheet", "sheet"),
      ],
    },
    {
      id: "tools",
      label: "Ferramentas",
      items: [placeholder("settings", "project-window")],
    },
    {
      id: "help",
      label: "Ajuda",
      items: [
        placeholder("manual", "project-window"),
        placeholder("shortcuts", "project-window"),
        separator("help-about-separator"),
        placeholder("about", "project-window"),
      ],
    },
  ];
}

function implemented(
  id: ProjectCommandId,
  context: ProjectCommandContext,
  onSelect: () => void,
  disabled?: boolean,
  checked?: boolean,
): ApplicationMenuCommand {
  const descriptor = projectCommandDescriptor(id);
  const binding = projectCommandBinding(id, context);
  if (binding?.availability !== "implemented") {
    throw new Error(`Comando ${id} não está implementado em ${context}.`);
  }
  return {
    availability: "implemented",
    context,
    disabled,
    checked,
    id,
    label: descriptor.label,
    onSelect,
    shortcut: projectCommandShortcutLabel(id),
    type: "command",
  };
}

function placeholder(
  id: ProjectCommandId,
  context: ProjectCommandContext,
): ApplicationMenuCommand {
  const descriptor = projectCommandDescriptor(id);
  const binding = projectCommandBinding(id, context);
  if (binding?.availability !== "placeholder") {
    throw new Error(`Comando ${id} não é um placeholder em ${context}.`);
  }
  return {
    availability: "placeholder",
    context,
    feature: binding.placeholderFeature,
    id,
    label: descriptor.label,
    shortcut: projectCommandShortcutLabel(id),
    type: "command",
  };
}

function separator(id: string) {
  return { id, type: "separator" as const };
}

function submenu(
  id: string,
  label: string,
  items: readonly ApplicationMenuCommand[],
) {
  return { id, items, label, type: "submenu" as const };
}
