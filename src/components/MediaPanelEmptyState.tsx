import type { MediaKind } from "../domain/project";
import { EmptyState } from "../ui";

interface MediaPanelEmptyStateProps {
  kind: MediaKind;
  reason: "catalog" | "filtered" | "folder";
}

interface EmptyStateContent {
  description: string;
  title: string;
}

const catalogContent: Record<MediaKind, EmptyStateContent> = {
  decorative: {
    description: "Use Importar para adicionar fundos e sobreposições.",
    title: "Nenhum decorativo importado",
  },
  photo: {
    description: "Use Importar para adicionar fotos ao projeto.",
    title: "Nenhuma foto importada",
  },
};

const filteredContent: EmptyStateContent = {
  description: "Tente outro nome ou altere o filtro.",
  title: "Nenhum item encontrado",
};

export function MediaPanelEmptyState({
  kind,
  reason,
}: MediaPanelEmptyStateProps) {
  const content = reason === "catalog" ? catalogContent[kind] : reason === "folder"
    ? { title: "Pasta vazia", description: "Use Mover para pasta… no menu das imagens." } : filteredContent;

  return (
    <EmptyState
      className={`media-empty-state media-empty-state--${reason}`}
      density="compact"
      description={content.description}
      title={content.title}
    />
  );
}
