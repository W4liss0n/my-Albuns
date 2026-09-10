import {
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent,
  type Ref,
} from "react";
import type {
  MediaPreview,
  MediaPreviewDemand,
} from "../application/projectPorts";
import { matchProjectCommandShortcut } from "../application/projectCommandCatalog";
import type { MediaPanelPersistentPreference } from "../application/workspacePreferences";

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
import { MediaPreviewCard } from "./MediaPreviewCard";
import { isTextEntryTarget } from "./isTextEntryTarget";
import "./MediaPanel.css";
import { MEDIA_PANEL_PRELOAD_MARGIN, mediaPanelViewportDemand } from "./mediaPanelViewport";

export interface MediaPanelHandle {
  planCatalog(mediaItems: readonly MediaCatalogItem[], mediaUsage: readonly MediaUsage[]): {
    demand: MediaPreviewDemand;
    commit(): void;
  };
}

type MediaPanelPreferenceMode =
  | {
      kind: "controlled";
      persistent: Readonly<Record<MediaKind, MediaPanelPersistentPreference>>;
      thumbnailSizes: Readonly<Record<MediaKind, number>>;
      onSortDirectionChange(
        mediaKind: MediaKind,
        sortDirection: MediaPanelPersistentPreference["sortDirection"],
      ): void;
      onUsageFilterChange(
        mediaKind: MediaKind,
        usageFilter: MediaPanelPersistentPreference["usageFilter"],
      ): void;
      onThumbnailSizeChange(mediaKind: MediaKind, size: number): void;
    }
  | {
      kind: "local";
      initial?: Partial<Record<MediaKind, MediaPanelViewPreferences>>;
    };

type MediaPanelPreviewSource =
  | {
      kind: "connected";
      previews: Readonly<Record<string, MediaPreview>>;
      onDemandChange(demand: MediaPreviewDemand): void;
    }
  | {
      kind: "static";
      previews?: Readonly<Record<string, MediaPreview>>;
    };

interface MediaPanelProps {
  ref?: Ref<MediaPanelHandle>;
  hidden?: boolean;
  mediaItems: readonly MediaCatalogItem[];
  mediaUsage: readonly MediaUsage[];
  onFillPhoto(mediaId: string): void;
  selectedMediaId: string | null;
  importPending?: boolean;
  onImportPhoto(): void;
  onSelectMedia(mediaId: string): void;
  onPhotoDragStart(mediaId: string): void;
  onPhotoDragEnd(): void;
  onRelinkMedia(mediaId: string): void;
  onRetryUnavailableMedia(mediaId: string): Promise<void>;
  relinkDisabled?: boolean;
  preferences: MediaPanelPreferenceMode;
  previewSource: MediaPanelPreviewSource;
}

const naturalNameCollator = new Intl.Collator("pt-BR", {
  numeric: true,
  sensitivity: "base",
});

export function MediaPanel({
  ref,
  hidden = false,
  mediaItems,
  mediaUsage,
  onFillPhoto,
  selectedMediaId,
  importPending = false,
  onImportPhoto,
  onSelectMedia,
  onPhotoDragStart,
  onPhotoDragEnd,
  onRelinkMedia,
  onRetryUnavailableMedia,
  relinkDisabled = false,
  preferences: preferenceMode,
  previewSource,
}: MediaPanelProps) {
  const mediaPreviews = previewSource.previews ?? {};
  const onMediaDemandChange =
    previewSource.kind === "connected" ? previewSource.onDemandChange : null;
  const controlledPersistent =
    preferenceMode.kind === "controlled" ? preferenceMode.persistent : null;
  const controlledThumbnailSizes =
    preferenceMode.kind === "controlled"
      ? preferenceMode.thumbnailSizes
      : null;
  const [activeMediaKind, setActiveMediaKind] =
    useState<MediaKind>("photo");
  const [searchByKind, setSearchByKind] = useState<Record<MediaKind, string>>({
    decorative: "",
    photo: "",
  });
  const [preferencesByKind, setPreferencesByKind] = useState<
    Record<MediaKind, MediaPanelViewPreferences>
  >(() => ({
    decorative: {
      ...createMediaPanelViewPreferences(),
      ...initialPreferences(preferenceMode, "decorative"),
    },
    photo: {
      ...createMediaPanelViewPreferences(),
      ...initialPreferences(preferenceMode, "photo"),
    },
  }));
  const [selectedMediaIds, setSelectedMediaIds] = useState<ReadonlySet<string>>(
    () => new Set(),
  );
  const [selectionAnchorId, setSelectionAnchorId] = useState<string | null>(
    null,
  );
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
  const visibleMediaItems = useMemo(() => filterMediaItems(
    activeMediaItems, mediaUsageById, search, sortDirection, usageFilter,
  ), [activeMediaItems, mediaUsageById, search, sortDirection, usageFilter]);
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
  const transparentDragImageRef = useRef<HTMLCanvasElement>(null);
  const observedDemandByKind = useRef<Record<MediaKind, MediaPreviewDemand>>({
    photo: { visibleMediaIds: [], preloadMediaIds: [] },
    decorative: { visibleMediaIds: [], preloadMediaIds: [] },
  });

  useImperativeHandle(ref, () => ({
    planCatalog(nextItems, nextUsage) {
      const ordered = filterMediaItems(
        nextItems.filter((media) => media.kind === activeMediaKind),
        new Map(nextUsage.map((usage) => [usage.mediaId, usage.count])),
        search, sortDirection, usageFilter,
      );
      const demand = mediaPanelViewportDemand(gridRef.current, ordered.map(({ id }) => id), thumbnailSize);
      const inactive = observedDemandByKind.current[activeMediaKind === "photo" ? "decorative" : "photo"];
      return {
        demand: { ...demand, preloadMediaIds: [
          ...demand.preloadMediaIds, ...inactive.visibleMediaIds, ...inactive.preloadMediaIds,
        ] },
        commit() { observedDemandByKind.current[activeMediaKind] = demand; },
      };
    },
  }));

  useEffect(() => {
    if (!controlledThumbnailSizes) return;
    setPreferencesByKind((current) => ({
      decorative: {
        ...current.decorative,
        thumbnailSize: controlledThumbnailSizes.decorative,
      },
      photo: {
        ...current.photo,
        thumbnailSize: controlledThumbnailSizes.photo,
      },
    }));
  }, [controlledThumbnailSizes]);

  useEffect(() => {
    if (!controlledPersistent) return;
    setPreferencesByKind((current) => ({
      decorative: {
        ...current.decorative,
        ...controlledPersistent.decorative,
      },
      photo: {
        ...current.photo,
        ...controlledPersistent.photo,
      },
    }));
  }, [controlledPersistent]);

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
    if (!selectedMediaId || !visibleMediaIdSet.has(selectedMediaId)) return;
    setSelectedMediaIds(new Set([selectedMediaId]));
    setSelectionAnchorId(selectedMediaId);
  }, [selectedMediaId, visibleMediaIdSet]);

  useEffect(() => {
    if (!onMediaDemandChange) return;
    return () => {
      onMediaDemandChange({ visibleMediaIds: [], preloadMediaIds: [] });
    };
  }, [onMediaDemandChange]);

  useEffect(() => {
    if (!onMediaDemandChange) return;
    const root = gridRef.current;
    const targets = root?.querySelectorAll<HTMLElement>("[data-media-id]");
    const snapshots = observedDemandByKind.current;
    for (const kind of ["photo", "decorative"] as const) {
      const allowed = new Set(
        kind === activeMediaKind
          ? visibleMediaIds
          : mediaItems.filter((media) => media.kind === kind).map(({ id }) => id),
      );
      snapshots[kind] = {
        visibleMediaIds: snapshots[kind].visibleMediaIds.filter((id) => allowed.has(id)),
        preloadMediaIds: snapshots[kind].preloadMediaIds.filter((id) => allowed.has(id)),
      };
    }
    // The native window is initially hidden. Its first intersection notification
    // can precede the first paint; geometry must establish demand independently.
    if (root?.clientWidth && root.clientHeight) {
      snapshots[activeMediaKind] = mediaPanelViewportDemand(root, visibleMediaIds, thumbnailSize);
    }
    const visible = new Set(snapshots[activeMediaKind].visibleMediaIds);
    const resident = new Set([
      ...visible,
      ...snapshots[activeMediaKind].preloadMediaIds,
    ]);
    let active = true;
    const emitDemand = () => {
      const observedVisible = visibleMediaIds.filter((id) => visible.has(id));
      const observedPreload = visibleMediaIds
        .filter(
          (mediaId) => resident.has(mediaId) && !visible.has(mediaId),
        );
      snapshots[activeMediaKind] = {
        visibleMediaIds: observedVisible,
        preloadMediaIds: observedPreload,
      };
      const inactive = snapshots[activeMediaKind === "photo" ? "decorative" : "photo"];
      onMediaDemandChange({
        visibleMediaIds: observedVisible,
        preloadMediaIds: [
          ...observedPreload,
          ...inactive.visibleMediaIds,
          ...inactive.preloadMediaIds,
        ],
      });
    };
    emitDemand();
    if (!root || !targets?.length || !("IntersectionObserver" in globalThis)) return;

    const update = (entries: IntersectionObserverEntry[], set: Set<string>) => {
      if (!active) return;
      if (root.clientWidth && root.clientHeight) {
        measureDemand();
        return;
      }
      for (const entry of entries) {
        const mediaId = (entry.target as HTMLElement).dataset.mediaId;
        if (!mediaId) continue;
        if (entry.isIntersecting) set.add(mediaId);
        else set.delete(mediaId);
      }
      emitDemand();
    };
    const measureDemand = () => {
      if (!active) return;
      const measured = mediaPanelViewportDemand(root, visibleMediaIds, thumbnailSize);
      visible.clear();
      resident.clear();
      measured.visibleMediaIds.forEach((id) => visible.add(id));
      [...measured.visibleMediaIds, ...measured.preloadMediaIds].forEach((id) => resident.add(id));
      emitDemand();
    };
    const visibleObserver = new IntersectionObserver(
      (entries) => update(entries, visible),
      { root, rootMargin: "0px", threshold: 0.01 },
    );
    const preloadObserver = new IntersectionObserver(
      (entries) => update(entries, resident),
      { root, rootMargin: `${MEDIA_PANEL_PRELOAD_MARGIN}px 0px`, threshold: 0.01 },
    );
    targets.forEach((target) => {
      visibleObserver.observe(target);
      preloadObserver.observe(target);
    });
    root.addEventListener("scroll", measureDemand, { passive: true });
    const resizeObserver = typeof ResizeObserver === "undefined" ? undefined : new ResizeObserver(measureDemand);
    resizeObserver?.observe(root);
    return () => {
      active = false;
      root.removeEventListener("scroll", measureDemand);
      resizeObserver?.disconnect();
      visibleObserver.disconnect();
      preloadObserver.disconnect();
    };
  }, [activeMediaKind, mediaItems, onMediaDemandChange, thumbnailSize, visibleMediaIds]);

  function updatePreferences(
    nextPreferences: Partial<MediaPanelViewPreferences>,
  ) {
    if (
      preferenceMode.kind === "controlled" &&
      nextPreferences.thumbnailSize !== undefined
    ) {
      preferenceMode.onThumbnailSizeChange(
        activeMediaKind,
        nextPreferences.thumbnailSize,
      );
    }
    if (
      preferenceMode.kind === "controlled" &&
      nextPreferences.sortDirection !== undefined
    ) {
      preferenceMode.onSortDirectionChange(
        activeMediaKind,
        nextPreferences.sortDirection,
      );
    }
    if (
      preferenceMode.kind === "controlled" &&
      nextPreferences.usageFilter !== undefined
    ) {
      preferenceMode.onUsageFilterChange(
        activeMediaKind,
        nextPreferences.usageFilter,
      );
    }
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
    onSelectMedia(mediaId);
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
      matchProjectCommandShortcut(event, "media-panel") !== "select-all" ||
      isTextEntryTarget(event.target)
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

  return (
    <section
      id="media-panel"
      className="media-panel"
      hidden={hidden}
      data-project-command-context="media-panel"
      aria-label="Painel de imagens"
      onKeyDown={selectAllVisibleMedia}
    >
      <canvas
        aria-hidden="true"
        height={1}
        ref={transparentDragImageRef}
        style={{
          position: "fixed",
          top: 0,
          left: 0,
          width: 1,
          height: 1,
          pointerEvents: "none",
        }}
        width={1}
      />
      <MediaPanelToolbar
        activeMediaKind={activeMediaKind}
        itemCount={activeMediaItems.length}
        preferences={preferences}
        search={search}
        importDisabled={relinkDisabled || importPending}
        importPending={importPending}
        onImportPhoto={onImportPhoto}
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
            const availabilityLabel = preview
              ? mediaAvailabilityLabel(preview)
              : null;
            const accessibleLabel = [
              media.name,
              isUsed ? "Já usada" : null,
              availabilityLabel,
            ]
              .filter(Boolean)
              .join(". ");
            return (
              <div className="media-card-shell" key={media.id}>
                <MediaPreviewCard
                aria-label={accessibleLabel}
                aria-pressed={isSelected}
                data-media-id={media.id}
                data-used={String(isUsed)}
                dimmed={isUsed}
                draggable={media.kind === "photo"}
                kind="media"
                media={media}
                previewUrl={preview?.url ?? undefined}
                selected={isSelected}
                onClick={(event) => selectMedia(media.id, event)}
                onContextMenu={() => selectMediaForContextMenu(media.id)}
                onDragStart={
                  media.kind === "photo"
                    ? (event) => {
                        event.dataTransfer.effectAllowed = "copy";
                        const dragImage = transparentDragImageRef.current;
                        if (dragImage) event.dataTransfer.setDragImage(dragImage, 0, 0);
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
                {availabilityLabel && (
                  <span
                    aria-label={availabilityLabel ?? undefined}
                    className="media-availability"
                    role="status"
                  >
                    {availabilityLabel}
                  </span>
                )}
                </MediaPreviewCard>
                {preview?.state === "absent" && (
                  <button
                    aria-label={`Religar arquivo de ${media.name}`}
                    className="media-recovery-action"
                    disabled={relinkDisabled}
                    type="button"
                    onClick={() => onRelinkMedia(media.id)}
                  >
                    Religar
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
          })
        )}
      </div>
    </section>
  );
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
      return null;
  }
}

function initialPreferences(
  mode: MediaPanelPreferenceMode,
  mediaKind: MediaKind,
): Partial<MediaPanelViewPreferences> {
  return mode.kind === "controlled"
    ? {
        ...mode.persistent[mediaKind],
        thumbnailSize: mode.thumbnailSizes[mediaKind],
      }
    : mode.initial?.[mediaKind] ?? {};
}

function normalizeSearchText(value: string) {
  return value
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLocaleLowerCase("pt-BR");
}

function filterMediaItems(
  items: readonly MediaCatalogItem[],
  usage: ReadonlyMap<string, number>,
  search: string,
  sortDirection: MediaPanelViewPreferences["sortDirection"],
  usageFilter: MediaUsageFilter,
) {
  const normalizedSearch = normalizeSearchText(search);
  const direction = sortDirection === "ascending" ? 1 : -1;
  return items.filter((media) =>
    passesUsageFilter(usage.get(media.id) ?? 0, usageFilter) &&
    normalizeSearchText(media.name).includes(normalizedSearch),
  ).sort((left, right) => direction * naturalNameCollator.compare(left.name, right.name));
}

function passesUsageFilter(
  usageCount: number,
  usageFilter: MediaUsageFilter,
) {
  if (usageFilter === "used") return usageCount > 0;
  if (usageFilter === "unused") return usageCount === 0;
  return true;
}
