import type { FrameStackAction } from "../domain/project";

export type ProjectCommandContext =
  | "welcome"
  | "project-window"
  | "sheet"
  | "frame"
  | "media-panel"
  | "media-folder"
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
    id: "toggle-photo-black-and-white",
    label: "Preto e branco",
    description: "Alterna o efeito Preto e branco das fotos selecionadas.",
    kind: "domain", contexts: ["frame", "frame-photo"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "rotate-photo-counterclockwise",
    label: "Girar 90° à esquerda",
    description: "Gira as fotos selecionadas 90° no sentido anti-horário.",
    kind: "domain", contexts: ["frame", "frame-photo"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "mirror-photo-horizontal",
    label: "Espelhar horizontalmente",
    description: "Alterna o espelhamento horizontal das fotos selecionadas.",
    kind: "domain", contexts: ["frame", "frame-photo"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "new-project",
    label: "Novo projeto…",
    description: "Inicia a criação de um novo projeto.",
    kind: "application",
    bindings: [
      implementedBinding("welcome"),
      implementedBinding("project-window"),
    ],
    shortcuts: [shortcut("n", "Ctrl+N", { ctrlKey: true })],
  }),
  command({
    id: "open-project",
    label: "Abrir projeto…",
    description: "Abre outro projeto existente.",
    kind: "application",
    bindings: [
      implementedBinding("welcome"),
      implementedBinding("project-window"),
    ],
    shortcuts: [shortcut("o", "Ctrl+O", { ctrlKey: true })],
  }),
  command({
    id: "save",
    label: "Salvar",
    description: "Salva as alterações do projeto.",
    kind: "application",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [shortcut("s", "Ctrl+S", { ctrlKey: true })],
  }),
  command({
    id: "save-as",
    label: "Salvar como…",
    description: "Salva uma cópia independente do projeto em outro arquivo.",
    kind: "application",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [
      shortcut("s", "Ctrl+Shift+S", { ctrlKey: true, shiftKey: true }),
    ],
  }),
  command({
    id: "export-sheet",
    label: "Exportar lâmina…",
    description: "Exporta a lâmina centralizada na área de edição.",
    kind: "application",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "export",
    label: "Exportar…",
    description: "Escolha o que exportar e onde salvar os arquivos.",
    kind: "application",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "close",
    label: "Fechar projeto",
    description: "Fecha o projeto e avisa se houver alterações não salvas.",
    kind: "application",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [shortcut("w", "Ctrl+W", { ctrlKey: true })],
  }),
  command({
    id: "undo",
    label: "Desfazer",
    description: "Desfaz a última alteração.",
    kind: "domain",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [shortcut("z", "Ctrl+Z", { ctrlKey: true })],
  }),
  command({
    id: "redo",
    label: "Refazer",
    description: "Refaz a alteração desfeita.",
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
    description: "Copia os quadros selecionados.",
    kind: "domain",
    contexts: ["frame"],
    availability: "implemented",
    shortcuts: [shortcut("c", "Ctrl+C", { ctrlKey: true })],
  }),
  command({
    id: "paste-frames",
    label: "Colar",
    description: "Cola quadros copiados na lâmina ativa.",
    kind: "domain",
    contexts: ["frame"],
    availability: "implemented",
    shortcuts: [shortcut("v", "Ctrl+V", { ctrlKey: true })],
  }),
  command({
    id: "swap-frame-contents",
    label: "Trocar conteúdo dos quadros",
    description: "Troca o conteúdo entre os quadros selecionados.",
    kind: "domain",
    contexts: ["frame"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "swap-sheet-sides",
    label: "Trocar lados",
    description: "Troca os quadros de página dentro da lâmina dupla.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "add-frame",
    label: "Adicionar quadro",
    description: "Adiciona um novo quadro à lâmina ativa.",
    kind: "domain",
    contexts: ["frame"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "delete-frames",
    label: "Excluir",
    description: "Exclui os quadros selecionados no Modo de edição da lâmina.",
    kind: "domain",
    contexts: ["frame"],
    availability: "implemented",
    shortcuts: [shortcut("delete", "Delete")],
  }),
  command({
    id: "import-media",
    label: "Importar",
    description: "Escolhe como importar imagens para a aba ativa do painel.",
    kind: "interface", contexts: ["media-panel"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "import-media-files",
    label: "Arquivos…",
    description: "Importa os arquivos de imagem escolhidos para a aba ativa do painel.",
    kind: "application", contexts: ["media-panel"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "import-media-folder",
    label: "Pasta…",
    description: "Importa as imagens da pasta escolhida para a aba ativa do painel.",
    kind: "application", contexts: ["media-panel"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "relink-media",
    label: "Localizar imagem…",
    description: "Escolha a pasta onde está a imagem.",
    kind: "application", contexts: ["media-panel"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "replace-media",
    label: "Substituir imagem",
    description: "Troca a imagem selecionada por outro arquivo.",
    kind: "application", contexts: ["media-panel"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "create-media-folder",
    label: "Nova pasta",
    description: "Cria uma pasta de organização na aba ativa do painel de imagens.",
    kind: "domain", contexts: ["media-panel"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "move-media-to-folder",
    label: "Mover para pasta…",
    description: "Organiza as imagens selecionadas na pasta escolhida.",
    kind: "domain", contexts: ["media-panel"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "rename-media-folder",
    label: "Renomear…",
    description: "Altera o nome da pasta de organização contextual.",
    kind: "domain", contexts: ["media-folder"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "delete-media-folder",
    label: "Excluir pasta",
    description: "Exclui a pasta de organização preservando suas imagens no projeto.",
    kind: "domain", contexts: ["media-folder"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "remove-media",
    label: "Remover",
    description: "Remove as imagens selecionadas na aba ativa do painel.",
    kind: "domain",
    contexts: ["media-panel"],
    availability: "implemented",
    shortcuts: [shortcut("delete", "Delete")],
  }),
  command({
    id: "bring-frames-to-front",
    label: "Trazer para frente",
    description: "Move os quadros selecionados para a frente.",
    kind: "domain",
    contexts: ["frame"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "advance-frames",
    label: "Avançar uma posição",
    description: "Avança os quadros selecionados uma posição.",
    kind: "domain",
    contexts: ["frame"],
    availability: "implemented",
    shortcuts: [shortcut("]", "Ctrl+]", { ctrlKey: true })],
  }),
  command({
    id: "recede-frames",
    label: "Recuar uma posição",
    description: "Recua os quadros selecionados uma posição.",
    kind: "domain",
    contexts: ["frame"],
    availability: "implemented",
    shortcuts: [shortcut("[", "Ctrl+[", { ctrlKey: true })],
  }),
  command({
    id: "send-frames-to-back",
    label: "Enviar para trás",
    description: "Move os quadros selecionados para trás.",
    kind: "domain",
    contexts: ["frame"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "save-frame-arrangement-as-layout",
    label: "Salvar disposição como layout",
    description: "Salva a disposição atual dos quadros como layout.",
    kind: "application",
    contexts: ["frame"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "select-all",
    label: "Selecionar tudo",
    description: "Seleciona todos os itens do contexto ativo.",
    kind: "interface",
    bindings: [
      implementedBinding("frame"),
      implementedBinding("media-panel"),
    ],
    shortcuts: [shortcut("a", "Ctrl+A", { ctrlKey: true })],
  }),
  command({
    id: "enter-sheet-editing",
    label: "Editar lâmina",
    description: "Isola a lâmina centralizada para editar seus quadros e fotos.",
    kind: "interface",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [shortcut("enter", "Enter")],
  }),
  command({
    id: "previous-sheet",
    label: "Lâmina anterior",
    description: "Centraliza a lâmina física anterior na área de edição.",
    kind: "interface",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [shortcut("arrowleft", "←")],
  }),
  command({
    id: "next-sheet",
    label: "Próxima lâmina",
    description: "Centraliza a próxima lâmina física na área de edição.",
    kind: "interface",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [shortcut("arrowright", "→")],
  }),
  command({
    id: "add-before",
    label: "Adicionar antes",
    description: "Adiciona uma lâmina antes da lâmina ativa.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "add-after",
    label: "Adicionar depois",
    description: "Adiciona uma lâmina depois da lâmina ativa.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "duplicate-sheet",
    label: "Duplicar lâmina",
    description: "Duplica a lâmina ativa.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "delete-sheet",
    label: "Excluir",
    description: "Exclui a lâmina ativa.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [shortcut("delete", "Delete")],
  }),
  command({
    id: "convert-edge",
    label: "Converter extremidade",
    description: "Converte a configuração da lâmina de extremidade.",
    kind: "domain",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "media-panel",
    label: "Painel de imagens",
    description: "Mostra ou oculta o painel de imagens.",
    kind: "interface",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "contextual-panel",
    label: "Painel contextual",
    description: "Mostra ou oculta o painel contextual.",
    kind: "interface",
    contexts: ["project-window"],
    availability: "implemented",
    shortcuts: [],
  }),
  command({
    id: "fit-sheet",
    label: "Ajustar lâmina",
    description: "Ajusta a lâmina ativa ao espaço disponível.",
    kind: "interface",
    contexts: ["sheet"],
    availability: "implemented",
    shortcuts: [shortcut("0", "Ctrl+0", { ctrlKey: true })],
  }),
  command({
    id: "canvas-zoom-in", label: "Ampliar lâmina",
    description: "Amplia a visualização da lâmina em edição.",
    kind: "interface", contexts: ["sheet"], availability: "implemented",
    shortcuts: [shortcut("+", "Ctrl++", { ctrlKey: true }),
      shortcut("+", undefined, { ctrlKey: true, shiftKey: true }),
      shortcut("=", undefined, { ctrlKey: true })],
  }),
  command({
    id: "canvas-zoom-out", label: "Reduzir lâmina",
    description: "Reduz a visualização até o enquadramento completo da lâmina.",
    kind: "interface", contexts: ["sheet"], availability: "implemented",
    shortcuts: [shortcut("-", "Ctrl+−", { ctrlKey: true })],
  }),
  command({
    id: "generate-projects",
    label: "Gerar projetos em lote…",
    description: "Cria projetos independentes usando o estado atual como modelo.",
    kind: "application", contexts: ["project-window"], availability: "implemented", shortcuts: [],
  }),
  command({
    id: "settings",
    label: "Configurações…",
    description: "Abre as Configurações globais do aplicativo.",
    kind: "application",
    contexts: ["project-window"],
    availability: "implemented",
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
    description: "Abre a foto contextual original no Photoshop.",
    kind: "application",
    contexts: ["frame-photo", "media-photo"],
    availability: "implemented",
    shortcuts: [shortcut("e", "Ctrl+E", { ctrlKey: true })],
  }),
] as const;

export type ProjectCommandId = (typeof DEFINITIONS)[number]["id"];

export const FRAME_STACK_COMMANDS = [
  { id: "bring-frames-to-front", action: "bringToFront" },
  { id: "advance-frames", action: "advance" },
  { id: "recede-frames", action: "recede" },
  { id: "send-frames-to-back", action: "sendToBack" },
] as const satisfies readonly { id: ProjectCommandId; action: FrameStackAction }[];

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
