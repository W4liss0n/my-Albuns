import { useState } from "react";

import type {
  MediaCatalogItem,
  MediaKind,
  MediaUsage,
} from "../domain/project";
import "./MediaPanel.css";

export interface MediaPanelProps {
  mediaItems: readonly MediaCatalogItem[];
  mediaUsage: readonly MediaUsage[];
  mediaPreviewUrls?: Readonly<Record<string, string>>;
  onFillPhoto(mediaId: string): void;
}

export function MediaPanel({
  mediaItems,
  mediaUsage,
  mediaPreviewUrls = {},
  onFillPhoto,
}: MediaPanelProps) {
  const [activeMediaKind, setActiveMediaKind] =
    useState<MediaKind>("photo");
  const mediaUsageById = new Map(
    mediaUsage.map((usage) => [usage.mediaId, usage.count]),
  );

  return (
    <section
      id="media-panel"
      className="media-panel"
      aria-label="Painel de imagens"
    >
      <div className="media-panel-head">
        <div className="media-tabs">
          <button
            className={activeMediaKind === "photo" ? "active" : undefined}
            type="button"
            onClick={() => setActiveMediaKind("photo")}
          >
            Fotos
          </button>
          <button
            className={
              activeMediaKind === "decorative" ? "active" : undefined
            }
            type="button"
            onClick={() => setActiveMediaKind("decorative")}
          >
            Decorativos
          </button>
        </div>
        <label className="media-search">
          <span aria-hidden="true">⌕</span>
          <input aria-label="Buscar imagens" placeholder="Buscar imagens" />
        </label>
      </div>
      <div className="media-strip">
        {mediaItems
          .filter((media) => media.kind === activeMediaKind)
          .map((media) => (
            <button
              className="media-card"
              type="button"
              key={media.id}
              onDoubleClick={
                media.kind === "photo"
                  ? () => onFillPhoto(media.id)
                  : undefined
              }
              title={
                media.kind === "photo"
                  ? "Duplo clique para preencher o placeholder mais à esquerda da Lâmina centralizada"
                  : undefined
              }
            >
              <span
                className="media-thumb"
                style={{
                  background: `linear-gradient(135deg, ${media.palette[0]}, ${media.palette[1]} 56%, ${media.palette[2]})`,
                }}
              >
                {mediaPreviewUrls[media.id] && (
                  <img
                    alt=""
                    draggable="false"
                    loading="lazy"
                    src={mediaPreviewUrls[media.id]}
                  />
                )}
              </span>
              <span className="media-meta">
                <strong>{media.name}</strong>
                <small>{mediaUsageById.get(media.id) ?? 0} usos</small>
              </span>
            </button>
          ))}
        {activeMediaKind === "photo" && (
          <div className="media-tip">
            <kbd>2×</kbd>
            <span>
              Preenche o placeholder mais à esquerda da Lâmina centralizada
            </span>
          </div>
        )}
      </div>
    </section>
  );
}
