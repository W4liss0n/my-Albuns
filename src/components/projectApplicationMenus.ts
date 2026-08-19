import type { ApplicationMenuGroup } from "./ApplicationMenuBar";

interface ProjectApplicationMenuOptions {
  canExport: boolean;
  canRedo: boolean;
  canUndo: boolean;
  closeProject(): void;
  exportSheet(): void;
  redo(): void;
  save(): void;
  undo(): void;
}

export function createProjectApplicationMenus({
  canExport,
  canRedo,
  canUndo,
  closeProject,
  exportSheet,
  redo,
  save,
  undo,
}: ProjectApplicationMenuOptions): readonly ApplicationMenuGroup[] {
  // PLACEHOLDER UI: these commands establish the accepted desktop information
  // architecture. Each placeholder stays disabled until its application port or
  // Project intent exists; commands already backed by a port remain functional.
  return [
    {
      id: "file",
      label: "Arquivo",
      items: [
        placeholder(
          "new-project",
          "Novo Projeto…",
          "new-project-from-project-window",
          "Ctrl+N",
        ),
        placeholder(
          "open-project",
          "Abrir Projeto…",
          "open-project-from-project-window",
          "Ctrl+O",
        ),
        separator("file-project-separator"),
        implemented("save", "Salvar", save, "Ctrl+S"),
        placeholder("save-as", "Salvar como…", "save-project-as", "Ctrl+Shift+S"),
        separator("file-export-separator"),
        implemented(
          "export-sheet",
          "Exportar Lâmina…",
          exportSheet,
          undefined,
          !canExport,
        ),
        placeholder("export", "Exportar…", "normal-project-export"),
        separator("file-close-separator"),
        implemented("close", "Fechar Projeto", closeProject, "Ctrl+W"),
      ],
    },
    {
      id: "edit",
      label: "Editar",
      items: [
        implemented("undo", "Desfazer", undo, "Ctrl+Z", !canUndo),
        implemented("redo", "Refazer", redo, "Ctrl+Shift+Z", !canRedo),
        separator("edit-clipboard-separator"),
        placeholder("copy-frames", "Copiar", "copy-frames", "Ctrl+C"),
        placeholder("paste-frames", "Colar", "paste-frames", "Ctrl+V"),
        separator("edit-frame-separator"),
        placeholder(
          "swap-frame-contents",
          "Trocar conteúdo dos Frames",
          "swap-selected-frame-contents",
        ),
        placeholder("add-frame", "Adicionar Frame", "add-frame"),
        submenu("arrange-frames", "Organizar", [
          placeholder(
            "bring-frames-to-front",
            "Trazer para frente",
            "bring-selected-frames-to-front",
          ),
          placeholder(
            "advance-frames",
            "Avançar uma posição",
            "advance-selected-frames",
            "Ctrl+]",
          ),
          placeholder(
            "recede-frames",
            "Recuar uma posição",
            "recede-selected-frames",
            "Ctrl+[",
          ),
          placeholder(
            "send-frames-to-back",
            "Enviar para trás",
            "send-selected-frames-to-back",
          ),
        ]),
        separator("edit-layout-separator"),
        placeholder(
          "save-frame-arrangement-as-layout",
          "Salvar disposição como Layout",
          "save-frame-arrangement-as-layout",
        ),
        placeholder(
          "select-all",
          "Selecionar tudo",
          "select-all-in-active-context",
          "Ctrl+A",
        ),
      ],
    },
    {
      id: "sheet",
      label: "Lâmina",
      items: [
        placeholder("add-before", "Adicionar antes", "add-sheet-before"),
        placeholder("add-after", "Adicionar depois", "add-sheet-after"),
        placeholder("duplicate-sheet", "Duplicar Lâmina", "duplicate-sheet"),
        placeholder("delete-sheet", "Excluir", "delete-sheet", "Delete"),
        separator("sheet-edge-separator"),
        placeholder(
          "convert-edge",
          "Converter extremidade",
          "convert-edge-sheet",
        ),
      ],
    },
    {
      id: "view",
      label: "Exibir",
      items: [
        placeholder("media-panel", "Painel de imagens", "toggle-media-panel"),
        placeholder(
          "contextual-panel",
          "Painel contextual",
          "toggle-contextual-panel",
        ),
        separator("view-canvas-separator"),
        placeholder("fit-sheet", "Ajustar Lâmina", "fit-sheet-in-edit-mode"),
      ],
    },
    {
      id: "tools",
      label: "Ferramentas",
      items: [
        placeholder(
          "settings",
          "Configurações…",
          "open-global-settings-from-project",
        ),
      ],
    },
    {
      id: "help",
      label: "Ajuda",
      items: [
        placeholder("manual", "Manual do MyAlbuns", "application-manual"),
        placeholder("shortcuts", "Atalhos de teclado", "keyboard-shortcuts"),
        separator("help-about-separator"),
        placeholder("about", "Sobre o MyAlbuns", "about-application"),
      ],
    },
  ];
}

function implemented(
  id: string,
  label: string,
  onSelect: () => void,
  shortcut?: string,
  disabled?: boolean,
) {
  return {
    availability: "implemented" as const,
    disabled,
    id,
    label,
    onSelect,
    shortcut,
    type: "command" as const,
  };
}

function placeholder(
  id: string,
  label: string,
  feature: string,
  shortcut?: string,
) {
  return {
    availability: "placeholder" as const,
    feature,
    id,
    label,
    shortcut,
    type: "command" as const,
  };
}

function separator(id: string) {
  return { id, type: "separator" as const };
}

function submenu(
  id: string,
  label: string,
  items: readonly ReturnType<typeof placeholder>[],
) {
  return { id, items, label, type: "submenu" as const };
}
