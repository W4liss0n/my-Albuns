import { useEffect, useMemo, useRef, useState } from "react";
import {
  Image as ImageIcon,
  Search,
  SlidersHorizontal,
} from "lucide-react";

import type {
  MediaPreview,
  MediaPreviewDemand,
} from "../application/projectPorts";

import type {
  MediaCatalogItem,
  MediaKind,
  MediaUsage,
} from "../domain/project";
import { AppIcon } from "../ui";
import "./MediaPanel.css";

export interface MediaPanelProps {
  mediaItems: readonly MediaCatalogItem[];
  mediaUsage: readonly MediaUsage[];
  mediaPreviews?: Readonly<Record<string, MediaPreview>>;
  onMediaDemandChange?(demand: MediaPreviewDemand): void;
  onFillPhoto(mediaId: string): void;
}

export function MediaPanel({
  mediaItems,
  mediaUsage,
  mediaPreviews = {},
  onMediaDemandChange,
  onFillPhoto,
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
        <div className="media-tabs" aria-label="Tipo de recurso">
          <button
            aria-label="Fotos"
            className={activeMediaKind === "photo" ? "active" : undefined}
            type="button"
            onClick={() => setActiveMediaKind("photo")}
          >
            <AppIcon icon={ImageIcon} size={16} />
          </button>
          <button
            aria-label="Decorativos"
            className={
              activeMediaKind === "decorative" ? "active" : undefined
            }
            type="button"
            onClick={() => setActiveMediaKind("decorative")}
          >
            <AppIcon icon={SlidersHorizontal} size={16} />
          </button>
        </div>
        <span className="media-kind-chip">
          {activeMediaKind === "photo" ? "Todas" : "Decorativos"}
          <small>{activeMediaItems.length}</small>
        </span>
        <label className="media-search">
          <AppIcon icon={Search} size={12} />
          <input aria-label="Buscar imagens" placeholder="Buscar…" />
        </label>
      </div>
      <div className="media-strip" ref={stripRef}>
        {activeMediaItems.map((media) => (
            <button
              className="media-card"
              type="button"
              key={media.id}
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
                {mediaPreviews[media.id]?.state === "unavailable" && (
                  <span
                    aria-label={
                      mediaPreviews[media.id].url
                        ? "Indisponível · prévia anterior"
                        : "Indisponível"
                    }
                    className="media-availability"
                    role="status"
                  >
                    {mediaPreviews[media.id].url
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
          ))}
      </div>
    </section>
  );
}

function mediaCardBackground(media: MediaCatalogItem) {
  const palette = media.palette ?? ["#26323A", "#53636D", "#A6B0B6"];
  return `linear-gradient(135deg, ${palette[0]}, ${palette[1]} 56%, ${palette[2]})`;
}
