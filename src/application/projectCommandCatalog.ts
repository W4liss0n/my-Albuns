export type ProjectCommandContext =
  | "welcome"
  | "project-window"
  | "sheet"
  | "frame"
  | "media-panel"
  | "frame-photo"
  | "media-photo";

export type ProjectCommandKind = "application" | "domain" | "interface";
export type ProjectCommandAvailability = "implemented" | "placeholder";

export type ProjectCommandContextBinding =
  | {
      availability: "implemented";
      context: ProjectCommandContext;
    }
  | {
      availability: "placeholder";
      context: ProjectCommandContext;
      placeholderFeature: string;
    };

export interface ProjectCommandShortcutEvent {
  altKey: boolean;
  ctrlKey: boolean;
  key: string;
  metaKey: boolean;
  shiftKey: boolean;
}

export interface ProjectCommandShortcut {
  altKey?: boolean;
  ctrlKey?: boolean;
  display?: string;
  key: string;
  shiftKey?: boolean;
}

export interface ProjectCommandDefinition<Id extends string = string> {
  bindings: readonly ProjectCommandContextBinding[];
  description: string;
  id: Id;
  kind: ProjectCommandKind;
  label: string;
  shortcuts: readonly ProjectCommandShortcut[];
}

interface ProjectCommandDefinitionBase<Id extends string> {
  description: string;
  id: Id;
  kind: ProjectCommandKind;
  label: string;
  shortcuts: readonly ProjectCommandShortcut[];
}

type ProjectCommandSourceDefinition<Id extends string> =
  | (ProjectCommandDefinitionBase<Id> & {
      availability: "implemented";
      contexts: readonly ProjectCommandContext[];
    })
  | (ProjectCommandDefinitionBase<Id> & {
      availability: "placeholder";
      contexts: readonly ProjectCommandContext[];
      placeholderFeature: string;
    })
  | (ProjectCommandDefinitionBase<Id> & {
      bindings: readonly ProjectCommandContextBinding[];
    });

const DEFINITIONS = [
  command({
    id: "new-project",
    label: "Novo Projeto…",
    description: "Inicia a criação de um novo Projeto.",
    kind: "application",
    bindings: [
      implementedBinding("welcome"),
      placeholderBinding(
        "project-window",
        "new-project-from-project-window",
      ),
    ],
    shortcuts: [shortcut("n", "Ctrl+N", { ctrlKey: true })],
  }),
  command({
    id: "open-project",
    label: "Abrir Projeto…",
    description: "Abre outro Projeto existente.",
    kind: "application",
    bindings: [
      implementedBinding("welcome"),
      placeholderBinding(
        "project-window",
        "open-project-from-project-window",
      ),
    ],
    shortcuts: [shortcut("o", "Ctrl+O", { ctrlKey: true })],
  }),
  command({
    id: "save",
    label: "Salvar",
    description: "Salva a revisão atual do Projeto.",
    kind: "application",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [shortcut("s", "Ctrl+S", { ctrlKey: true })],
  }),
  command({
    id: "save-as",
    label: "Salvar como…",
    description: "Salva o Projeto em uma nova Localização.",
    kind: "application",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [
      shortcut("s", "Ctrl+Shift+S", { ctrlKey: true, shiftKey: true }),
    ],
  }),
  command({
    id: "export-sheet",
    label: "Exportar Lâmina…",
    description: "Exporta a Lâmina centralizada no Canvas.",
    kind: "application",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "export",
    label: "Exportar…",
    description: "Abre o fluxo normal de Exportação do Projeto.",
    kind: "application",
    contexts: ["project-window"],
    availability: "placeholder",
    placeholderFeature: "normal-project-export",
    shortcuts: [],
  }),
  command({
    id: "close",
    label: "Fechar Projeto",
    description: "Fecha a Janela do Projeto com proteção de alterações.",
    kind: "application",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [shortcut("w", "Ctrl+W", { ctrlKey: true })],
  }),
  command({
    id: "undo",
    label: "Desfazer",
    description: "Desfaz a última alteração de domínio do Projeto.",
    kind: "domain",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [shortcut("z", "Ctrl+Z", { ctrlKey: true })],
  }),
  command({
    id: "redo",
    label: "Refazer",
    description: "Refaz a última alteração de domínio desfeita.",
    kind: "domain",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [
      shortcut("z", "Ctrl+Shift+Z", { ctrlKey: true, shiftKey: true }),
      shortcut("y", undefined, { ctrlKey: true }),
    ],
  }),
  command({
    id: "copy-frames",
    label: "Copiar",
    description: "Copia os Frames selecionados.",
    kind: "domain",
    contexts: ["frame"],
    availability: "placeholder",
    placeholderFeature: "copy-frames",
    shortcuts: [shortcut("c", "Ctrl+C", { ctrlKey: true })],
  }),
  command({
    id: "paste-frames",
    label: "Colar",
    description: "Cola Frames copiados na Lâmina ativa.",
    kind: "domain",
    contexts: ["frame"],
    availability: "placeholder",
    placeholderFeature: "paste-frames",
    shortcuts: [shortcut("v", "Ctrl+V", { ctrlKey: true })],
  }),
  command({
    id: "swap-frame-contents",
    label: "Trocar conteúdo dos Frames",
    description: "Troca o conteúdo entre os Frames selecionados.",
    kind: "domain",
    contexts: ["frame"],
    availability: "placeholder",
    placeholderFeature: "swap-selected-frame-contents",
    shortcuts: [],
  }),
  command({
    id: "add-frame",
    label: "Adicionar Frame",
    description: "Adiciona um novo Frame à Lâmina ativa.",
    kind: "domain",
    contexts: ["frame"],
    availability: "placeholder",
    placeholderFeature: "add-frame",
    shortcuts: [],
  }),
  command({
    id: "bring-frames-to-front",
    label: "Trazer para frente",
    description: "Move os Frames selecionados para a frente.",
    kind: "domain",
    contexts: ["frame"],
    availability: "placeholder",
    placeholderFeature: "bring-selected-frames-to-front",
    shortcuts: [],
  }),
  command({
    id: "advance-frames",
    label: "Avançar uma posição",
    description: "Avança os Frames selecionados uma posição.",
    kind: "domain",
    contexts: ["frame"],
    availability: "placeholder",
    placeholderFeature: "advance-selected-frames",
    shortcuts: [shortcut("]", "Ctrl+]", { ctrlKey: true })],
  }),
  command({
    id: "recede-frames",
    label: "Recuar uma posição",
    description: "Recua os Frames selecionados uma posição.",
    kind: "domain",
    contexts: ["frame"],
    availability: "placeholder",
    placeholderFeature: "recede-selected-frames",
    shortcuts: [shortcut("[", "Ctrl+[", { ctrlKey: true })],
  }),
  command({
    id: "send-frames-to-back",
    label: "Enviar para trás",
    description: "Move os Frames selecionados para trás.",
    kind: "domain",
    contexts: ["frame"],
    availability: "placeholder",
    placeholderFeature: "send-selected-frames-to-back",
    shortcuts: [],
  }),
  command({
    id: "save-frame-arrangement-as-layout",
    label: "Salvar disposição como Layout",
    description: "Salva a disposição atual dos Frames como Layout.",
    kind: "application",
    contexts: ["frame"],
    availability: "placeholder",
    placeholderFeature: "save-frame-arrangement-as-layout",
    shortcuts: [],
  }),
  command({
    id: "select-all",
    label: "Selecionar tudo",
    description: "Seleciona todos os itens do contexto ativo.",
    kind: "interface",
    bindings: [
      placeholderBinding("frame", "select-all-in-active-context"),
      implementedBinding("media-panel"),
    ],
    shortcuts: [shortcut("a", "Ctrl+A", { ctrlKey: true })],
  }),
  command({
    id: "add-before",
    label: "Adicionar antes",
    description: "Adiciona uma Lâmina antes da Lâmina ativa.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "add-after",
    label: "Adicionar depois",
    description: "Adiciona uma Lâmina depois da Lâmina ativa.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "duplicate-sheet",
    label: "Duplicar Lâmina",
    description: "Duplica a Lâmina ativa.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "placeholder",
    placeholderFeature: "duplicate-sheet",
    shortcuts: [],
  }),
  command({
    id: "delete-sheet",
    label: "Excluir",
    description: "Exclui a Lâmina ativa.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [shortcut("delete", "Delete")],
  }),
  command({
    id: "convert-edge",
    label: "Converter extremidade",
    description: "Converte a configuração da Lâmina de extremidade.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "placeholder",
    placeholderFeature: "convert-edge-sheet",
    shortcuts: [],
  }),
  command({
    id: "media-panel",
    label: "Painel de imagens",
    description: "Mostra ou oculta o Painel de imagens.",
    kind: "interface",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "contextual-panel",
    label: "Painel contextual",
    description: "Mostra ou oculta o Painel contextual.",
    kind: "interface",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "fit-sheet",
    label: "Ajustar Lâmina",
    description: "Ajusta a Lâmina ativa ao espaço disponível.",
    kind: "interface",
    contexts: ["sheet"],
    availability: "placeholder",
    placeholderFeature: "fit-sheet-in-edit-mode",
    shortcuts: [],
  }),
  command({
    id: "settings",
    label: "Configurações…",
    description: "Abre as Configurações globais do aplicativo.",
    kind: "application",
    contexts: ["project-window"],
    availability: "placeholder",
    placeholderFeature: "open-global-settings-from-project",
    shortcuts: [],
  }),
  command({
    id: "manual",
    label: "Manual do MyAlbuns",
    description: "Abre o Manual do MyAlbuns.",
    kind: "application",
    contexts: ["project-window"],
    availability: "placeholder",
    placeholderFeature: "application-manual",
    shortcuts: [],
  }),
  command({
    id: "shortcuts",
    label: "Atalhos de teclado",
    description: "Mostra os atalhos de teclado do aplicativo.",
    kind: "interface",
    contexts: ["project-window"],
    availability: "placeholder",
    placeholderFeature: "keyboard-shortcuts",
    shortcuts: [],
  }),
  command({
    id: "about",
    label: "Sobre o MyAlbuns",
    description: "Mostra informações sobre o MyAlbuns.",
    kind: "application",
    contexts: ["project-window"],
    availability: "placeholder",
    placeholderFeature: "about-application",
    shortcuts: [],
  }),
  command({
    id: "open-in-photoshop",
    label: "Abrir no Photoshop",
    description: "Abre a Foto contextual original no Photoshop.",
    kind: "application",
    contexts: ["frame-photo", "media-photo"],
    availability: "placeholder",
    placeholderFeature: "open-linked-photo-in-photoshop",
    shortcuts: [shortcut("e", "Ctrl+E", { ctrlKey: true })],
  }),
] as const;

export type ProjectCommandId = (typeof DEFINITIONS)[number]["id"];

export const PROJECT_COMMAND_CATALOG: readonly ProjectCommandDefinition<ProjectCommandId>[] =
  DEFINITIONS;

export function projectCommandDescriptor(commandId: ProjectCommandId) {
  const definition = DEFINITIONS.find(
    (candidate) => candidate.id === commandId,
  );
  if (!definition) {
    throw new Error(`Comando canônico ausente: ${commandId}`);
  }
  return definition as ProjectCommandDefinition<ProjectCommandId>;
}

export function projectCommandShortcutLabel(commandId: ProjectCommandId) {
  return projectCommandDescriptor(commandId).shortcuts.find(
    (candidate) => candidate.display !== undefined,
  )?.display;
}

export function projectCommandShortcutAria(commandId: ProjectCommandId) {
  const candidate = projectCommandDescriptor(commandId).shortcuts.find(
    (shortcut) => shortcut.display !== undefined,
  );
  if (!candidate) return undefined;
  return [
    candidate.ctrlKey ? "Control" : null,
    candidate.altKey ? "Alt" : null,
    candidate.shiftKey ? "Shift" : null,
    candidate.key.length === 1
      ? candidate.key.toLocaleUpperCase("en-US")
      : candidate.key,
  ]
    .filter(Boolean)
    .join("+");
}

export function projectCommandBinding(
  commandId: ProjectCommandId,
  context: ProjectCommandContext,
) {
  return projectCommandDescriptor(commandId).bindings.find(
    (binding) => binding.context === context,
  );
}

export function matchProjectCommandShortcut(
  event: ProjectCommandShortcutEvent,
  context: ProjectCommandContext,
): ProjectCommandId | null {
  const eventKey = event.key.toLowerCase();
  for (const definition of DEFINITIONS) {
    if (!definition.bindings.some((binding) => binding.context === context)) {
      continue;
    }
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
  definition: ProjectCommandSourceDefinition<Id>,
): ProjectCommandDefinition<Id> {
  if ("bindings" in definition) return definition;
  const bindings = definition.contexts.map((context) =>
    definition.availability === "implemented"
      ? implementedBinding(context)
      : placeholderBinding(context, definition.placeholderFeature),
  );
  return {
    bindings,
    description: definition.description,
    id: definition.id,
    kind: definition.kind,
    label: definition.label,
    shortcuts: definition.shortcuts,
  };
}

function implementedBinding(
  context: ProjectCommandContext,
): ProjectCommandContextBinding {
  return { availability: "implemented", context };
}

function placeholderBinding(
  context: ProjectCommandContext,
  placeholderFeature: string,
): ProjectCommandContextBinding {
  return { availability: "placeholder", context, placeholderFeature };
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
