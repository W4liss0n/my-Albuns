import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import type {
  MediaPreview,
  MediaPreviewDemand,
} from "../application/projectPorts";

import type {
  MediaCatalogItem,
  MediaKind,
  MediaUsage,
} from "../domain/project";
import {
  createMediaPanelViewPreferences,
  type MediaPanelViewPreferences,
  type MediaUsageFilter,
} from "../state/mediaPanelPreferences";
import { MediaPanelEmptyState } from "./MediaPanelEmptyState";
import { MediaPanelToolbar } from "./MediaPanelToolbar";
import "./MediaPanel.css";

export interface MediaPanelProps {
  mediaItems: readonly MediaCatalogItem[];
  mediaUsage: readonly MediaUsage[];
  mediaPreviews?: Readonly<Record<string, MediaPreview>>;
  onMediaDemandChange?(demand: MediaPreviewDemand): void;
  onFillPhoto(mediaId: string): void;
}

const naturalNameCollator = new Intl.Collator("pt-BR", {
  numeric: true,
  sensitivity: "base",
});

export function MediaPanel({
  mediaItems,
  mediaUsage,
  mediaPreviews = {},
  onMediaDemandChange,
  onFillPhoto,
}: MediaPanelProps) {
  const [activeMediaKind, setActiveMediaKind] =
    useState<MediaKind>("photo");
  const [searchByKind, setSearchByKind] = useState<Record<MediaKind, string>>({
    decorative: "",
    photo: "",
  });
  const [preferencesByKind, setPreferencesByKind] = useState<
    Record<MediaKind, MediaPanelViewPreferences>
  >(() => ({
    decorative: createMediaPanelViewPreferences(),
    photo: createMediaPanelViewPreferences(),
  }));
  const mediaUsageById = useMemo(
    () => new Map(mediaUsage.map((usage) => [usage.mediaId, usage.count])),
    [mediaUsage],
  );
  const activeMediaItems = useMemo(
    () => mediaItems.filter((media) => media.kind === activeMediaKind),
    [activeMediaKind, mediaItems],
  );
  const search = searchByKind[activeMediaKind];
  const preferences = preferencesByKind[activeMediaKind];
  const { sortDirection, thumbnailSize, usageFilter } = preferences;
  const visibleMediaItems = useMemo(() => {
    const normalizedSearch = normalizeSearchText(search);
    const direction = sortDirection === "ascending" ? 1 : -1;
    return activeMediaItems
      .filter((media) => {
        const usageCount = mediaUsageById.get(media.id) ?? 0;
        return (
          passesUsageFilter(usageCount, usageFilter) &&
          normalizeSearchText(media.name).includes(normalizedSearch)
        );
      })
      .sort(
        (left, right) =>
          direction * naturalNameCollator.compare(left.name, right.name),
      );
  }, [activeMediaItems, mediaUsageById, search, sortDirection, usageFilter]);
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!onMediaDemandChange) return;
    const root = gridRef.current;
    const targets = root?.querySelectorAll<HTMLElement>("[data-media-id]");
    onMediaDemandChange({ visibleMediaIds: [], preloadMediaIds: [] });
    if (!root || !targets?.length || !("IntersectionObserver" in globalThis)) {
      return;
    }

    const visible = new Set<string>();
    const resident = new Set<string>();
    const emitDemand = () => {
      const visibleMediaIds = visibleMediaItems
        .map(({ id }) => id)
        .filter((mediaId) => visible.has(mediaId));
      const preloadMediaIds = visibleMediaItems
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
  }, [onMediaDemandChange, visibleMediaItems]);

  function updatePreferences(
    nextPreferences: Partial<MediaPanelViewPreferences>,
  ) {
    setPreferencesByKind((current) => ({
      ...current,
      [activeMediaKind]: {
        ...current[activeMediaKind],
        ...nextPreferences,
      },
    }));
  }

  return (
    <section
      id="media-panel"
      className="media-panel"
      aria-label="Painel de imagens"
    >
      <MediaPanelToolbar
        activeMediaKind={activeMediaKind}
        itemCount={activeMediaItems.length}
        preferences={preferences}
        search={search}
        onActiveMediaKindChange={setActiveMediaKind}
        onPreferencesChange={updatePreferences}
        onSearchChange={(nextSearch) =>
          setSearchByKind((current) => ({
            ...current,
            [activeMediaKind]: nextSearch,
          }))
        }
      />
      <div
        aria-label={
          activeMediaKind === "photo"
            ? "Grade de Fotos"
            : "Grade de Decorativos"
        }
        className="media-grid"
        ref={gridRef}
        role="group"
        style={
          {
            "--media-thumbnail-size": `${thumbnailSize}px`,
          } as CSSProperties
        }
      >
        {activeMediaItems.length === 0 ? (
          <MediaPanelEmptyState kind={activeMediaKind} reason="catalog" />
        ) : visibleMediaItems.length === 0 ? (
          <MediaPanelEmptyState kind={activeMediaKind} reason="filtered" />
        ) : (
          visibleMediaItems.map((media) => {
            const usageCount = mediaUsageById.get(media.id) ?? 0;
            const preview = mediaPreviews[media.id];
            const usageLabel =
              usageCount === 0
                ? null
                : usageCount === 1
                  ? "Usada 1 vez"
                  : `Usada ${usageCount} vezes`;
            const availabilityLabel =
              preview?.state !== "unavailable"
                ? null
                : preview.url
                  ? "Indisponível · prévia anterior"
                  : "Indisponível";
            const accessibleLabel = [
              media.name,
              usageLabel,
              availabilityLabel,
            ]
              .filter(Boolean)
              .join(". ");
            return (
              <button
                aria-label={accessibleLabel}
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
                <span className="media-thumb">
                  {preview?.url && (
                    <img
                      alt=""
                      draggable="false"
                      loading="lazy"
                      src={preview.url}
                    />
                  )}
                  {usageCount > 0 && (
                    <span
                      aria-label={
                        usageLabel ?? undefined
                      }
                      className="media-usage-badge"
                    >
                      <span aria-hidden="true" className="media-usage-dot" />
                      {usageCount === 1 ? "1 uso" : `${usageCount} usos`}
                    </span>
                  )}
                  {preview?.state === "unavailable" && (
                    <span
                      aria-label={availabilityLabel ?? undefined}
                      className="media-availability"
                      role="status"
                    >
                      {preview.url
                        ? "Indisponível · prévia anterior"
                        : "Indisponível"}
                    </span>
                  )}
                </span>
                <span className="media-meta">
                  <strong>{media.name}</strong>
                </span>
              </button>
            );
          })
        )}
      </div>
    </section>
  );
}

function normalizeSearchText(value: string) {
  return value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLocaleLowerCase("pt-BR");
}

function passesUsageFilter(
  usageCount: number,
  usageFilter: MediaUsageFilter,
) {
  if (usageFilter === "used") return usageCount > 0;
  if (usageFilter === "unused") return usageCount === 0;
  return true;
}
