import { useEffect, useMemo, useRef, useState } from "react";

import type {
  MediaPreview,
  MediaPreviewDemand,
} from "../application/projectPorts";

import type {
  MediaCatalogItem,
  MediaKind,
  MediaUsage,
} from "../domain/project";
import "./MediaPanel.css";

export interface MediaPanelProps {
  mediaItems: readonly MediaCatalogItem[];
  mediaUsage: readonly MediaUsage[];
  mediaPreviews?: Readonly<Record<string, MediaPreview>>;
  onMediaDemandChange?(demand: MediaPreviewDemand): void;
  onFillPhoto(mediaId: string): void;
  onRelinkMedia(mediaId: string): void;
  relinkDisabled?: boolean;
}

export function MediaPanel({
  mediaItems,
  mediaUsage,
  mediaPreviews = {},
  onMediaDemandChange,
  onFillPhoto,
  onRelinkMedia,
  relinkDisabled = false,
}: MediaPanelProps) {
  const [activeMediaKind, setActiveMediaKind] =
    useState<MediaKind>("photo");
  const mediaUsageById = new Map(
    mediaUsage.map((usage) => [usage.mediaId, usage.count]),
  );
  const activeMediaItems = useMemo(
    () => mediaItems.filter((media) => media.kind === activeMediaKind),
    [activeMediaKind, mediaItems],
  );
  const stripRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!onMediaDemandChange) return;
    const root = stripRef.current;
    const targets = root?.querySelectorAll<HTMLElement>("[data-media-id]");
    onMediaDemandChange({ visibleMediaIds: [], preloadMediaIds: [] });
    if (!root || !targets?.length || !("IntersectionObserver" in globalThis)) {
      return;
    }

    const visible = new Set<string>();
    const resident = new Set<string>();
    const emitDemand = () => {
      const visibleMediaIds = activeMediaItems
        .map(({ id }) => id)
        .filter((mediaId) => visible.has(mediaId));
      const preloadMediaIds = activeMediaItems
        .map(({ id }) => id)
        .filter(
          (mediaId) => resident.has(mediaId) && !visible.has(mediaId),
        );
      onMediaDemandChange({ visibleMediaIds, preloadMediaIds });
    };
    const update = (entries: IntersectionObserverEntry[], set: Set<string>) => {
      for (const entry of entries) {
        const mediaId = (entry.target as HTMLElement).dataset.mediaId;
        if (!mediaId) continue;
        if (entry.isIntersecting) set.add(mediaId);
        else set.delete(mediaId);
      }
      emitDemand();
    };
    const visibleObserver = new IntersectionObserver(
      (entries) => update(entries, visible),
      { root, rootMargin: "0px", threshold: 0.01 },
    );
    const preloadObserver = new IntersectionObserver(
      (entries) => update(entries, resident),
      { root, rootMargin: "122px 0px", threshold: 0.01 },
    );
    targets.forEach((target) => {
      visibleObserver.observe(target);
      preloadObserver.observe(target);
    });
    return () => {
      visibleObserver.disconnect();
      preloadObserver.disconnect();
    };
  }, [activeMediaItems, onMediaDemandChange]);

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
      <div className="media-strip" ref={stripRef}>
        {activeMediaItems.map((media) => (
          <div className="media-card-shell" key={media.id}>
            <button
              className="media-card"
              type="button"
              data-media-id={media.id}
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
                  background: mediaCardBackground(media),
                }}
              >
                {mediaPreviews[media.id]?.url && (
                  <img
                    alt=""
                    draggable="false"
                    loading="lazy"
                    src={mediaPreviews[media.id].url ?? undefined}
                  />
                )}
                {mediaPreviews[media.id]?.state !== "ready" &&
                  mediaPreviews[media.id] && (
                  <span
                    aria-label={
                      mediaPreviews[media.id].state === "absent"
                        ? mediaPreviews[media.id].url
                          ? "Arquivo ausente · prévia anterior"
                          : "Arquivo ausente"
                        : mediaPreviews[media.id].url
                          ? "Indisponível · prévia anterior"
                          : "Indisponível"
                    }
                    className="media-availability"
                    role="status"
                  >
                    {mediaPreviews[media.id].state === "absent"
                      ? mediaPreviews[media.id].url
                        ? "Arquivo ausente · prévia anterior"
                        : "Arquivo ausente"
                      : mediaPreviews[media.id].url
                        ? "Indisponível · prévia anterior"
                        : "Indisponível"}
                  </span>
                )}
              </span>
              <span className="media-meta">
                <strong>{media.name}</strong>
                <small>{mediaUsageById.get(media.id) ?? 0} usos</small>
              </span>
            </button>
            {mediaPreviews[media.id]?.state === "absent" && (
              <button
                aria-label={`Religar arquivo de ${media.name}`}
                className="media-relink"
                disabled={relinkDisabled}
                type="button"
                onClick={() => onRelinkMedia(media.id)}
              >
                Religar arquivo
              </button>
            )}
          </div>
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

function mediaCardBackground(media: MediaCatalogItem) {
  const palette = media.palette ?? ["#26323A", "#53636D", "#A6B0B6"];
  return `linear-gradient(135deg, ${palette[0]}, ${palette[1]} 56%, ${palette[2]})`;
}
