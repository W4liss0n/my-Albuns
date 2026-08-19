import {
  Image as ImageIcon,
  SearchX,
  Sparkles,
  type LucideIcon,
} from "lucide-react";

import type { MediaKind } from "../domain/project";
import { AppIcon, EmptyState } from "../ui";

interface MediaPanelEmptyStateProps {
  kind: MediaKind;
  reason: "catalog" | "filtered";
}

interface EmptyStateContent {
  description: string;
  eyebrow: string;
  icon: LucideIcon;
  title: string;
}

const catalogContent: Record<MediaKind, EmptyStateContent> = {
  decorative: {
    description: "As Imagens decorativas importadas aparecerão aqui.",
    eyebrow: "Decorativos",
    icon: Sparkles,
    title: "Nenhum Decorativo importado",
  },
  photo: {
    description: "As Fotos importadas para este Projeto aparecerão aqui.",
    eyebrow: "Fotos",
    icon: ImageIcon,
    title: "Nenhuma Foto importada",
  },
};

const filteredContent: EmptyStateContent = {
  description: "Ajuste a Busca ou o Filtro de uso para ver outros itens.",
  eyebrow: "Resultados",
  icon: SearchX,
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
      eyebrow={content.eyebrow}
      icon={<AppIcon icon={content.icon} size={18} />}
      title={content.title}
    />
  );
}
