import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent,
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
  const [selectedMediaIds, setSelectedMediaIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [selectionAnchorId, setSelectionAnchorId] = useState<string | null>(
    null,
  );
  const [intrinsicPreviewSizes, setIntrinsicPreviewSizes] = useState<
    Readonly<Record<string, IntrinsicPreviewSize>>
  >({});
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
  const visibleMediaIds = useMemo(
    () => visibleMediaItems.map(({ id }) => id),
    [visibleMediaItems],
  );
  const visibleMediaIdSet = useMemo(
    () => new Set(visibleMediaIds),
    [visibleMediaIds],
  );
  const emptyStateReason =
    activeMediaItems.length === 0
      ? "catalog"
      : visibleMediaItems.length === 0
        ? "filtered"
        : null;
  const gridRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    setSelectedMediaIds((current) => {
      const visibleSelection = new Set(
        [...current].filter((mediaId) => visibleMediaIdSet.has(mediaId)),
      );
      return visibleSelection.size === current.size ? current : visibleSelection;
    });
    setSelectionAnchorId((current) =>
      current && visibleMediaIdSet.has(current) ? current : null,
    );
  }, [visibleMediaIdSet]);

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

  function selectMedia(
    mediaId: string,
    event: MouseEvent<HTMLButtonElement>,
  ) {
    if (
      event.shiftKey &&
      selectionAnchorId &&
      visibleMediaIdSet.has(selectionAnchorId)
    ) {
      const anchorIndex = visibleMediaIds.indexOf(selectionAnchorId);
      const selectedIndex = visibleMediaIds.indexOf(mediaId);
      const rangeStart = Math.min(anchorIndex, selectedIndex);
      const rangeEnd = Math.max(anchorIndex, selectedIndex);
      setSelectedMediaIds(
        new Set(visibleMediaIds.slice(rangeStart, rangeEnd + 1)),
      );
      return;
    }

    if (event.ctrlKey || event.metaKey) {
      setSelectedMediaIds((current) => {
        const next = new Set(current);
        if (next.has(mediaId)) next.delete(mediaId);
        else next.add(mediaId);
        return next;
      });
      return;
    }

    setSelectedMediaIds(new Set([mediaId]));
    setSelectionAnchorId(mediaId);
  }

  function selectAllVisibleMedia(event: KeyboardEvent<HTMLElement>) {
    if (
      !(event.ctrlKey || event.metaKey) ||
      event.key.toLocaleLowerCase("pt-BR") !== "a" ||
      (event.target as HTMLElement).matches(
        "input, textarea, select, [contenteditable='true']",
      )
    ) {
      return;
    }
    event.preventDefault();
    setSelectedMediaIds(new Set(visibleMediaIds));
    setSelectionAnchorId((current) =>
      current && visibleMediaIdSet.has(current)
        ? current
        : (visibleMediaIds[0] ?? null),
    );
  }

  function selectMediaForContextMenu(mediaId: string) {
    if (selectedMediaIds.has(mediaId)) return;
    setSelectedMediaIds(new Set([mediaId]));
    setSelectionAnchorId(mediaId);
  }

  function clearSelectionFromGridBackground(
    event: MouseEvent<HTMLDivElement>,
  ) {
    if ((event.target as HTMLElement).closest("[data-media-id]")) return;
    setSelectedMediaIds(new Set());
    setSelectionAnchorId(null);
    event.currentTarget.focus({ preventScroll: true });
  }

  function captureIntrinsicPreviewSize(
    mediaId: string,
    previewUrl: string,
    image: HTMLImageElement,
  ) {
    const { naturalHeight, naturalWidth } = image;
    if (naturalWidth <= 0 || naturalHeight <= 0) return;
    setIntrinsicPreviewSizes((current) => {
      const previous = current[mediaId];
      if (
        previous?.previewUrl === previewUrl &&
        previous.width === naturalWidth &&
        previous.height === naturalHeight
      ) {
        return current;
      }
      return {
        ...current,
        [mediaId]: {
          height: naturalHeight,
          previewUrl,
          width: naturalWidth,
        },
      };
    });
  }

  return (
    <section
      id="media-panel"
      className="media-panel"
      aria-label="Painel de imagens"
      onKeyDown={selectAllVisibleMedia}
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
        data-empty={emptyStateReason ?? undefined}
        onClick={clearSelectionFromGridBackground}
        ref={gridRef}
        role="group"
        style={
          {
            "--media-thumbnail-size": `${thumbnailSize}px`,
          } as CSSProperties
        }
        tabIndex={-1}
      >
        {emptyStateReason ? (
          <MediaPanelEmptyState
            kind={activeMediaKind}
            reason={emptyStateReason}
          />
        ) : (
          visibleMediaItems.map((media) => {
            const usageCount = mediaUsageById.get(media.id) ?? 0;
            const isUsed = usageCount > 0;
            const isSelected = selectedMediaIds.has(media.id);
            const preview = mediaPreviews[media.id];
            const intrinsicPreviewSize = intrinsicPreviewSizes[media.id];
            const previewGeometry = mediaPreviewGeometry(
              media,
              intrinsicPreviewSize?.previewUrl === preview?.url
                ? intrinsicPreviewSize
                : undefined,
            );
            const availabilityLabel =
              preview?.state !== "unavailable"
                ? null
                : preview.url
                  ? "Indisponível · prévia anterior"
                  : "Indisponível";
            const accessibleLabel = [
              media.name,
              isUsed ? "Já usada" : null,
              availabilityLabel,
            ]
              .filter(Boolean)
              .join(". ");
            return (
              <button
                aria-label={accessibleLabel}
                aria-pressed={isSelected}
                className="media-card"
                type="button"
                key={media.id}
                data-media-id={media.id}
                data-selected={String(isSelected)}
                data-used={String(isUsed)}
                onClick={(event) => selectMedia(media.id, event)}
                onContextMenu={() => selectMediaForContextMenu(media.id)}
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
                  data-has-preview={String(Boolean(preview?.url))}
                  data-portrait={String(previewGeometry.isPortrait)}
                  style={
                    {
                      "--media-aspect-ratio": previewGeometry.aspectRatio,
                    } as CSSProperties
                  }
                >
                  {preview?.url && (
                    <img
                      alt=""
                      draggable="false"
                      loading="lazy"
                      onLoad={(event) => {
                        if (!preview.url || hasSourceDimensions(media)) return;
                        captureIntrinsicPreviewSize(
                          media.id,
                          preview.url,
                          event.currentTarget,
                        );
                      }}
                      src={preview.url}
                    />
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
              </button>
            );
          })
        )}
      </div>
    </section>
  );
}

interface MediaPreviewGeometry {
  aspectRatio: string;
  isPortrait: boolean;
}

interface IntrinsicPreviewSize {
  height: number;
  previewUrl: string;
  width: number;
}

function mediaPreviewGeometry(
  media: MediaCatalogItem,
  intrinsicSize?: IntrinsicPreviewSize,
): MediaPreviewGeometry {
  const { sourceHeightPx, sourceWidthPx } = media;
  const width = hasSourceDimensions(media)
    ? sourceWidthPx
    : intrinsicSize?.width;
  const height = hasSourceDimensions(media)
    ? sourceHeightPx
    : intrinsicSize?.height;
  if (!width || !height) return { aspectRatio: "1 / 1", isPortrait: false };
  return {
    aspectRatio: `${width} / ${height}`,
    isPortrait: height > width,
  };
}

function hasSourceDimensions(
  media: MediaCatalogItem,
): media is MediaCatalogItem & {
  sourceHeightPx: number;
  sourceWidthPx: number;
} {
  return (
    media.sourceWidthPx !== null &&
    media.sourceHeightPx !== null &&
    media.sourceWidthPx > 0 &&
    media.sourceHeightPx > 0
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
