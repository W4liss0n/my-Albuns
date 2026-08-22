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
  selectedMediaId?: string | null;
  onMediaDemandChange?(demand: MediaPreviewDemand): void;
  onFillPhoto(mediaId: string): void;
  onImportPhoto(): void;
  onSelectMedia(mediaId: string): void;
  onPhotoDragStart(mediaId: string): void;
  onPhotoDragEnd(): void;
  onRelinkMedia(mediaId: string): void;
  onRetryUnavailableMedia(mediaId: string): Promise<void>;
  relinkDisabled?: boolean;
}

function mediaAvailabilityLabel(preview: MediaPreview) {
  const previous = preview.url ? " · prévia anterior" : "";
  switch (preview.state) {
    case "absent":
      return `Arquivo ausente${previous}`;
    case "unavailable":
      return `Indisponível${previous}`;
    case "cache_unavailable":
      return `Prévia indisponível${previous}`;
    case "ready":
      return "";
  }
}

export function MediaPanel({
  mediaItems,
  mediaUsage,
  mediaPreviews = {},
  selectedMediaId = null,
  onMediaDemandChange,
  onFillPhoto,
  onImportPhoto,
  onSelectMedia,
  onPhotoDragStart,
  onPhotoDragEnd,
  onRelinkMedia,
  onRetryUnavailableMedia,
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
  const transparentDragImageRef = useRef<HTMLCanvasElement>(null);
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
      <canvas
        ref={transparentDragImageRef}
        aria-hidden="true"
        width={1}
        height={1}
        style={{
          position: "fixed",
          top: 0,
          left: 0,
          width: 1,
          height: 1,
          pointerEvents: "none",
        }}
      />
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
        {activeMediaKind === "photo" && (
          <button
            className="media-import"
            disabled={relinkDisabled}
            type="button"
            onClick={onImportPhoto}
          >
            Importar JPEG…
          </button>
        )}
        <label className="media-search">
          <span aria-hidden="true">⌕</span>
          <input aria-label="Buscar imagens" placeholder="Buscar imagens" />
        </label>
      </div>
      <div className="media-strip" ref={stripRef}>
        {activeMediaItems.map((media) => {
          const preview = mediaPreviews[media.id];
          const availabilityLabel =
            preview && preview.state !== "ready"
              ? mediaAvailabilityLabel(preview)
              : null;
          return (
            <div className="media-card-shell" key={media.id}>
              <button
                aria-pressed={selectedMediaId === media.id}
                className={`media-card${
                  selectedMediaId === media.id ? " selected" : ""
                }`}
                type="button"
                data-media-id={media.id}
                draggable={media.kind === "photo"}
                onClick={() => onSelectMedia(media.id)}
                onDragStart={
                  media.kind === "photo"
                    ? (event) => {
                        event.dataTransfer.effectAllowed = "copy";
                        const transparentDragImage = transparentDragImageRef.current;
                        if (!transparentDragImage) {
                          throw new Error(
                            "A imagem transparente de arraste não está disponível.",
                          );
                        }
                        event.dataTransfer.setDragImage(
                          transparentDragImage,
                          0,
                          0,
                        );
                        event.dataTransfer.setData(
                          "application/x-myalbuns-photo",
                          media.id,
                        );
                        onPhotoDragStart(media.id);
                      }
                    : undefined
                }
                onDragEnd={
                  media.kind === "photo" ? onPhotoDragEnd : undefined
                }
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
                  {preview?.url && (
                    <img
                      alt=""
                      draggable="false"
                      loading="lazy"
                      src={preview.url}
                    />
                  )}
                  {availabilityLabel && (
                    <span
                      aria-label={availabilityLabel}
                      className="media-availability"
                      role="status"
                    >
                      {availabilityLabel}
                    </span>
                  )}
                </span>
                <span className="media-meta">
                  <strong>{media.name}</strong>
                  <small>{mediaUsageById.get(media.id) ?? 0} usos</small>
                </span>
              </button>
              {preview?.state === "absent" && (
                <button
                  aria-label={`Religar arquivo de ${media.name}`}
                  className="media-recovery-action"
                  disabled={relinkDisabled}
                  type="button"
                  onClick={() => onRelinkMedia(media.id)}
                >
                  Religar arquivo
                </button>
              )}
              {preview?.state === "unavailable" && (
                <button
                  aria-label={`Tentar novamente o arquivo de ${media.name}`}
                  className="media-recovery-action"
                  type="button"
                  onClick={() => void onRetryUnavailableMedia(media.id)}
                >
                  Tentar novamente
                </button>
              )}
            </div>
          );
        })}
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
