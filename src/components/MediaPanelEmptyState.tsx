import type { MediaKind } from "../domain/project";
import { EmptyState } from "../ui";

interface MediaPanelEmptyStateProps {
  kind: MediaKind;
  reason: "catalog" | "filtered";
}

interface EmptyStateContent {
  description: string;
  title: string;
}

const catalogContent: Record<MediaKind, EmptyStateContent> = {
  decorative: {
    description: "As Imagens decorativas importadas aparecerão aqui.",
    title: "Nenhum Decorativo importado",
  },
  photo: {
    description: "As Fotos importadas para este Projeto aparecerão aqui.",
    title: "Nenhuma Foto importada",
  },
};

const filteredContent: EmptyStateContent = {
  description: "Ajuste a Busca ou o Filtro de uso para ver outros itens.",
  title: "Nenhum item encontrado",
};

export function MediaPanelEmptyState({
  kind,
  reason,
}: MediaPanelEmptyStateProps) {
  const content = reason === "catalog" ? catalogContent[kind] : filteredContent;

  return (
    <EmptyState
      className={`media-empty-state media-empty-state--${reason}`}
      density="compact"
      description={content.description}
      title={content.title}
    />
  );
}
