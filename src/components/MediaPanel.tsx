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
  MediaImportSelection,
  MediaFileInfo,
  MediaPreviewDemand,
} from "../application/projectPorts";
import { matchProjectCommandShortcut, projectCommandDescriptor, projectCommandShortcutLabel } from "../application/projectCommandCatalog";
import { ContextMenuSurface } from "../ui/ContextMenuSurface";
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
import { useMediaFileDrop } from "./useMediaFileDrop";
import { useMediaDragGesture, type MediaDrag } from "./useMediaDragGesture";
import "./MediaPanel.css";
import { MEDIA_PANEL_PRELOAD_MARGIN, mediaPanelViewportDemand } from "./mediaPanelViewport";

export interface MediaPanelHandle {
  showAbsent(): void;
  planCatalog(mediaItems: readonly MediaCatalogItem[], mediaUsage: readonly MediaUsage[]): {
    demand: MediaPreviewDemand;
    commit(): void;
  };
}

type MediaPanelPreferenceMode =
  | {
      kind: "controlled";
      activeKind: MediaKind;
      onActiveKindChange(mediaKind: MediaKind): void;
      onSortKeyChange(mediaKind: MediaKind, sortKey: MediaPanelPersistentPreference["sortKey"]): void;
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
  mediaFiles?: Readonly<Record<string, MediaFileInfo>>;
  onFillPhoto(mediaId: string): void;
  onApplyDecorative(mediaId: string, role: import("../domain/project").DecorativeRole): void;
  selectionRequest?: { mediaId: string } | null;
  importPending?: boolean;
  onImportMedia(selection: MediaImportSelection): void;
  onRemoveMedia(mediaIds: readonly string[]): void;
  dropPort?: import("../application/projectPorts").MediaDropPort;
  onMediaDragChange(drag: MediaDrag | null): void;
  dragThreshold?: import("../application/projectPorts").PointerDragThreshold | null;
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
const EMPTY_MEDIA_FILES: Readonly<Record<string, MediaFileInfo>> = {};

export function MediaPanel({
  ref,
  hidden = false,
  mediaItems,
  mediaUsage,
  mediaFiles = EMPTY_MEDIA_FILES,
  onFillPhoto,
  onApplyDecorative,
  selectionRequest,
  importPending = false,
  onImportMedia,
  onRemoveMedia,
  dropPort,
  onMediaDragChange,
  dragThreshold = { x: 5, y: 5 },
  onRelinkMedia,
  onRetryUnavailableMedia,
  relinkDisabled = false,
  preferences: preferenceMode,
  previewSource,
}: MediaPanelProps) {
  const mediaPreviews = previewSource.previews ?? {};
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);
  const onMediaDemandChange =
    previewSource.kind === "connected" ? previewSource.onDemandChange : null;
  const controlledPersistent =
    preferenceMode.kind === "controlled" ? preferenceMode.persistent : null;
  const controlledThumbnailSizes =
    preferenceMode.kind === "controlled"
      ? preferenceMode.thumbnailSizes
      : null;
  const [missingReview, setMissingReview] = useState<{
    activeKind: MediaKind;
    searches: Record<MediaKind, string>;
    usageFilters: Record<MediaKind, MediaUsageFilter>;
  } | null>(null);
  const [missingOnlyByKind, setMissingOnlyByKind] = useState<Record<MediaKind, boolean>>({ photo: false, decorative: false });
  const [localActiveMediaKind, setLocalActiveMediaKind] =
    useState<MediaKind>("photo");
  const preferredActiveMediaKind = preferenceMode.kind === "controlled" ? preferenceMode.activeKind : localActiveMediaKind;
  const activeMediaKind = missingReview?.activeKind ?? preferredActiveMediaKind;
  useEffect(() => { setContextMenu(null); }, [activeMediaKind, hidden]);
  function setActiveMediaKind(activeKind: MediaKind) {
    if (missingReview) setMissingReview({ ...missingReview, activeKind });
    else if (preferenceMode.kind === "controlled") preferenceMode.onActiveKindChange(activeKind);
    else setLocalActiveMediaKind(activeKind);
  }
  const fileInformation = useMemo(() => {
    const result = { ...mediaFiles };
    for (const media of mediaItems) {
      const preview = mediaPreviews[media.id];
      if (!result[media.id] && (preview?.state === "absent" || preview?.state === "unavailable")) {
        result[media.id] = { mediaId: media.id, state: preview.state, createdAtMs: null, modifiedAtMs: null };
      }
    }
    return result;
  }, [mediaFiles, mediaItems, previewSource.previews]);
  const missingCounts = useMemo(() => {
    const counts = { photo: 0, decorative: 0 };
    for (const media of mediaItems) if (fileInformation[media.id]?.state === "absent") counts[media.kind] += 1;
    return counts;
  }, [mediaItems, fileInformation]);
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
  const handledSelectionRequest = useRef<typeof selectionRequest>(null);
  const mediaUsageById = useMemo(
    () => new Map(mediaUsage.map((usage) => [usage.mediaId, usage.count])),
    [mediaUsage],
  );
  const usageDetailsById = useMemo(() => new Map(mediaUsage.map((usage) => [usage.mediaId, mediaUsageLabel(usage)])), [mediaUsage]);
  const activeMediaItems = useMemo(
    () => mediaItems.filter((media) => media.kind === activeMediaKind),
    [activeMediaKind, mediaItems],
  );
  const search = (missingReview?.searches ?? searchByKind)[activeMediaKind];
  const storedPreferences = preferencesByKind[activeMediaKind];
  const preferences = missingReview ? { ...storedPreferences, usageFilter: missingReview.usageFilters[activeMediaKind] } : storedPreferences;
  const missingOnly = missingReview !== null || missingOnlyByKind[activeMediaKind];
  const { sortKey, sortDirection, thumbnailSize, usageFilter } = preferences;
  const visibleMediaItems = useMemo(() => filterMediaItems(
    activeMediaItems, mediaUsageById, search, sortKey, sortDirection, usageFilter, fileInformation, missingOnly,
  ), [activeMediaItems, mediaUsageById, search, sortKey, sortDirection, usageFilter, fileInformation, missingOnly]);
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
  const mediaDrag = useMediaDragGesture({ threshold: dragThreshold, disabled: Boolean(hidden) || importPending || relinkDisabled, onChange: onMediaDragChange });
  const panelHostRef = useRef<HTMLElement>(null);
  const fileDrop = useMediaFileDrop({ port: dropPort, host: panelHostRef,
    hidden: Boolean(hidden), disabled: importPending || relinkDisabled,
    mediaKind: activeMediaKind, onImport: onImportMedia });
  const observedDemandByKind = useRef<Record<MediaKind, MediaPreviewDemand>>({
    photo: { visibleMediaIds: [], preloadMediaIds: [] },
    decorative: { visibleMediaIds: [], preloadMediaIds: [] },
  });

  useImperativeHandle(ref, () => ({
    showAbsent() {
      if (!missingCounts.photo && !missingCounts.decorative) return;
      setMissingReview({ activeKind: missingCounts[activeMediaKind] ? activeMediaKind : activeMediaKind === "photo" ? "decorative" : "photo",
        searches: { photo: "", decorative: "" }, usageFilters: { photo: "all", decorative: "all" } });
    },
    planCatalog(nextItems, nextUsage) {
      const ordered = filterMediaItems(
        nextItems.filter((media) => media.kind === activeMediaKind),
        new Map(nextUsage.map((usage) => [usage.mediaId, usage.count])),
        search, sortKey, sortDirection, usageFilter, fileInformation, missingOnly,
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
    if (selectionRequest === handledSelectionRequest.current) return;
    handledSelectionRequest.current = selectionRequest;
    if (!selectionRequest || !visibleMediaIdSet.has(selectionRequest.mediaId)) return;
    setSelectedMediaIds(new Set([selectionRequest.mediaId]));
    setSelectionAnchorId(selectionRequest.mediaId);
  }, [selectionRequest, visibleMediaIdSet]);

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
    if (missingReview && nextPreferences.usageFilter !== undefined) {
      setMissingReview({ ...missingReview, usageFilters: { ...missingReview.usageFilters, [activeMediaKind]: nextPreferences.usageFilter } });
      const { usageFilter: _filter, ...remaining } = nextPreferences;
      nextPreferences = remaining;
    }
    if (preferenceMode.kind === "controlled" && nextPreferences.sortKey !== undefined) {
      preferenceMode.onSortKeyChange(activeMediaKind, nextPreferences.sortKey);
    }
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
    if (isTextEntryTarget(event.target)) return;
    if (matchProjectCommandShortcut(event, "media-panel") === "remove-media") {
      event.preventDefault();
      event.stopPropagation();
      if (!relinkDisabled && !importPending) onRemoveMedia([...selectedMediaIds]);
      setContextMenu(null);
      return;
    }
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
      ref={panelHostRef}
      className={`media-panel${fileDrop.over ? " media-panel--file-drop" : ""}`}
      hidden={hidden}
      tabIndex={-1}
      data-project-command-context="media-panel"
      aria-label="Painel de imagens"
      onKeyDown={selectAllVisibleMedia}
    >
      {fileDrop.over && <div className="media-file-drop-hint" role="status">Solte para importar em {activeMediaKind === "photo" ? "Fotos" : "Decorativos"}</div>}
      {fileDrop.error && <div className="media-file-drop-error" role="status">{fileDrop.error}</div>}
      <MediaPanelToolbar
        activeMediaKind={activeMediaKind}
        missingCounts={missingCounts}
        missingOnly={missingOnly}
        reviewingMissing={missingReview !== null}
        onMissingOnlyChange={(value) => {
          if (missingReview && !value) setMissingReview(null);
          else setMissingOnlyByKind((current) => ({ ...current, [activeMediaKind]: value }));
        }}
        itemCount={activeMediaItems.length}
        preferences={preferences}
        search={search}
        importDisabled={relinkDisabled || importPending}
        importPending={importPending}
        onImportMedia={(kind) => onImportMedia({ mediaKind: activeMediaKind, source: { kind } })}
        onActiveMediaKindChange={setActiveMediaKind}
        onPreferencesChange={updatePreferences}
        onSearchChange={(nextSearch) => {
          if (missingReview) setMissingReview({ ...missingReview, searches: { ...missingReview.searches, [activeMediaKind]: nextSearch } });
          else setSearchByKind((current) => ({
            ...current,
            [activeMediaKind]: nextSearch,
          }));
        }}
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
            const cachedPreview = mediaPreviews[media.id];
            const file = fileInformation[media.id];
            const preview = file && file.state !== "available" ? {
              mediaId: media.id, state: file.state, url: cachedPreview?.url ?? null,
            } : cachedPreview;
            const availabilityLabel = preview
              ? mediaAvailabilityLabel(preview)
              : null;
            const accessibleLabel = [
              media.name,
              isUsed ? "Já usada" : null,
              usageDetailsById.get(media.id),
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
                draggable={false}
                kind="media"
                media={media}
                previewUrl={preview?.url ?? undefined}
                selected={isSelected}
                onClick={(event) => { if (!mediaDrag.suppressClick()) selectMedia(media.id, event); }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  selectMediaForContextMenu(media.id);
                  setContextMenu({ x: event.clientX, y: event.clientY });
                }}
                onPointerDown={(event) => mediaDrag.start(media.id, media.kind, event)}
                onDoubleClick={(event) => {
                  if (relinkDisabled || importPending) return;
                  if (media.kind === "photo") onFillPhoto(media.id);
                  else onApplyDecorative(media.id, event.shiftKey ? "overlay" : "background");
                }}
                title={[media.name, usageDetailsById.get(media.id), media.kind === "photo"
                  ? "Duplo clique para preencher o placeholder mais à esquerda da Lâmina centralizada"
                  : "Duplo clique aplica Fundo. Shift + duplo clique aplica Overlay."].filter(Boolean).join("\n")}
              >
                {isUsed && <span aria-hidden="true" className="media-usage-badge" title={usageDetailsById.get(media.id)}>{usageCount}</span>}
                {availabilityLabel && (
                  <span
                    aria-label={availabilityLabel ?? undefined}
                    className="media-availability"
                    role="status"
                  >
                    {preview?.state === "absent" ? "Ausente" : availabilityLabel}
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
      {contextMenu && <ContextMenuSurface label="Ações das imagens" position={contextMenu}
        onDismiss={() => { setContextMenu(null); panelHostRef.current?.focus({ preventScroll: true }); }}>
        <button type="button" role="menuitem" disabled={relinkDisabled || importPending || selectedMediaIds.size === 0}
          onClick={() => { setContextMenu(null); onRemoveMedia([...selectedMediaIds]); panelHostRef.current?.focus({ preventScroll: true }); }}>
          <span>{projectCommandDescriptor("remove-media").label}</span><kbd aria-hidden="true">{projectCommandShortcutLabel("remove-media")}</kbd>
        </button>
      </ContextMenuSurface>}
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
  sortKey: MediaPanelViewPreferences["sortKey"],
  sortDirection: MediaPanelViewPreferences["sortDirection"],
  usageFilter: MediaUsageFilter,
  files: Readonly<Record<string, MediaFileInfo>>,
  missingOnly: boolean,
) {
  const normalizedSearch = normalizeSearchText(search);
  const direction = sortDirection === "ascending" ? 1 : -1;
  return items.filter((media) =>
    (!missingOnly || files[media.id]?.state === "absent") &&
    passesUsageFilter(usage.get(media.id) ?? 0, usageFilter) &&
    normalizeSearchText(media.name).includes(normalizedSearch),
  ).sort((left, right) => {
    const leftFile = files[left.id];
    const rightFile = files[right.id];
    const absent = Number(leftFile?.state === "absent") - Number(rightFile?.state === "absent");
    if (absent) return absent;
    if (sortKey !== "name") {
      const leftDate = sortKey === "createdAt" ? leftFile?.createdAtMs : leftFile?.modifiedAtMs;
      const rightDate = sortKey === "createdAt" ? rightFile?.createdAtMs : rightFile?.modifiedAtMs;
      if (leftDate != null && rightDate != null && leftDate !== rightDate) return direction * (leftDate - rightDate);
      if ((leftDate == null) !== (rightDate == null)) return leftDate == null ? 1 : -1;
    }
    return direction * naturalNameCollator.compare(left.name, right.name);
  });
}

function passesUsageFilter(
  usageCount: number,
  usageFilter: MediaUsageFilter,
) {
  if (usageFilter === "used") return usageCount > 0;
  if (usageFilter === "unused") return usageCount === 0;
  return true;
}

function mediaUsageLabel(usage: MediaUsage): string {
  if (!usage.breakdown) return usage.count ? `${usage.count} ${usage.count === 1 ? "uso" : "usos"}` : "";
  const { frames, backgrounds, overlays, albumBackgrounds, albumOverlays } = usage.breakdown;
  return [
    frames ? `${frames} ${frames === 1 ? "Frame" : "Frames"}` : "",
    backgrounds ? `${backgrounds} ${backgrounds === 1 ? "Fundo" : "Fundos"}` : "",
    overlays ? `${overlays} ${overlays === 1 ? "Overlay" : "Overlays"}` : "",
    albumBackgrounds ? `${albumBackgrounds} ${albumBackgrounds === 1 ? "padrão" : "padrões"} de Fundo` : "",
    albumOverlays ? `${albumOverlays} ${albumOverlays === 1 ? "padrão" : "padrões"} de Overlay` : "",
  ].filter(Boolean).join(" · ");
}
