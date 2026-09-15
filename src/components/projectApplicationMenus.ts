import {
  FRAME_STACK_COMMANDS,
  projectCommandBinding,
  projectCommandDescriptor,
  projectCommandShortcutLabel,
  type ProjectCommandContext,
  type ProjectCommandId,
} from "../application/projectCommandCatalog";
import type { FrameStackAction } from "../domain/project";
import type {
  ApplicationMenuCommand,
  ApplicationMenuGroup,
} from "./ApplicationMenuBar";

interface ProjectApplicationMenuOptions {
  selectAllFrames(): void;
  canSelectAllFrames: boolean;
  generateProjects?(): void;
  openSettings?(): void;
  saveLayout(): void;
  canSaveLayout: boolean;
  copyFrames(): void;
  pasteFrames(): void;
  canCopyFrames: boolean;
  canPasteFrames: boolean;
  addFrame(): void;
  canAddFrame: boolean;
  arrangeFrames(action: FrameStackAction): void;
  canArrangeFrames: boolean;
  swapFrameContents(): void;
  canSwapFrameContents: boolean;
  addSheetAfter(): void;
  addSheetBefore(): void;
  canAddAfter: boolean;
  canAddBefore: boolean;
  canConvertEdge: boolean;
  canDelete: boolean;
  canDuplicate: boolean;
  canExport: boolean;
  canRedo: boolean;
  canUndo: boolean;
  contextualPanelVisible: boolean;
  closeProject(): void;
  convertEdge(): void;
  deleteSheet(): void;
  duplicateSheet(): void;
  exportSheet(): void;
  exportAlbum(): void;
  mediaPanelVisible: boolean;
  redo(): void;
  save(): void;
  saveAs(): void;
  structuralCommandsDisabled: boolean;
  undo(): void;
  toggleContextualPanel(): void;
  toggleMediaPanel(): void;
}

export function createProjectApplicationMenus({
  selectAllFrames,
  canSelectAllFrames,
  generateProjects,
  openSettings,
  saveLayout,
  canSaveLayout,
  copyFrames,
  pasteFrames,
  canCopyFrames,
  canPasteFrames,
  addFrame,
  canAddFrame,
  arrangeFrames,
  canArrangeFrames,
  swapFrameContents,
  canSwapFrameContents,
  addSheetAfter,
  addSheetBefore,
  canAddAfter,
  canAddBefore,
  canConvertEdge,
  canDelete,
  canDuplicate,
  canExport,
  canRedo,
  canUndo,
  contextualPanelVisible,
  closeProject,
  convertEdge,
  deleteSheet,
  duplicateSheet,
  exportSheet,
  exportAlbum,
  mediaPanelVisible,
  redo,
  save,
  saveAs,
  structuralCommandsDisabled,
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
        implemented("export", "project-window", exportAlbum, !canExport),
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
        implemented("copy-frames", "frame", copyFrames, !canCopyFrames),
        implemented("paste-frames", "frame", pasteFrames, !canPasteFrames),
        separator("edit-frame-separator"),
        implemented("swap-frame-contents", "frame", swapFrameContents, !canSwapFrameContents),
        implemented("add-frame", "frame", addFrame, !canAddFrame),
        submenu("arrange-frames", "Organizar", FRAME_STACK_COMMANDS.map(({ id, action }) =>
          implemented(id, "frame", () => arrangeFrames(action), !canArrangeFrames))),
        separator("edit-layout-separator"),
        { ...implemented("save-frame-arrangement-as-layout", "frame", saveLayout, !canSaveLayout),
          title: canSaveLayout ? "Salvar a disposição dos Frames em Personalizados." : "Entre no Modo de edição de uma Lâmina com ao menos um Frame." },
        implemented("select-all", "frame", selectAllFrames, !canSelectAllFrames),
      ],
    },
    {
      id: "sheet",
      label: "Lâmina",
      items: [
        implemented(
          "add-before",
          "sheet",
          addSheetBefore,
          structuralCommandsDisabled || !canAddBefore,
        ),
        implemented(
          "add-after",
          "sheet",
          addSheetAfter,
          structuralCommandsDisabled || !canAddAfter,
        ),
        implemented("duplicate-sheet", "sheet", duplicateSheet,
          structuralCommandsDisabled || !canDuplicate),
        implemented(
          "delete-sheet",
          "sheet",
          deleteSheet,
          structuralCommandsDisabled || !canDelete,
        ),
        separator("sheet-edge-separator"),
        implemented(
          "convert-edge",
          "sheet",
          convertEdge,
          structuralCommandsDisabled || !canConvertEdge,
        ),
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
      items: [
        implemented("generate-projects", "project-window", () => generateProjects?.(), !generateProjects),
        separator("tools-settings-separator"),
        implemented("settings", "project-window", () => openSettings?.(), !openSettings),
      ],
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
): Extract<ApplicationMenuCommand, { availability: "implemented" }> {
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
