import {
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent,
  type Ref,
} from "react";
import { ImageOff } from "lucide-react";
import type {
  MediaPreview,
  MediaImportSelection,
  MediaFileInfo,
  MediaPreviewDemand,
} from "../application/projectPorts";
import { matchProjectCommandShortcut, projectCommandDescriptor, projectCommandShortcutLabel } from "../application/projectCommandCatalog";
import { ContextMenuSurface } from "../ui/ContextMenuSurface";
import { AppIcon } from "../ui/AppIcon";
import type { MediaPanelPersistentPreference } from "../application/workspacePreferences";

import type {
  MediaFolder,
  MediaFolderEdit,
  MediaCatalogItem,
  MediaKind,
  MediaUsage,
} from "../domain/project";
import {
  createMediaPanelTabPreferences,
  MEDIA_THUMBNAIL_DEFAULT_SIZE,
  type MediaPanelViewPreferences,
  type MediaUsageFilter,
} from "../state/mediaPanelPreferences";
import { MediaPanelEmptyState } from "./MediaPanelEmptyState";
import { MediaFolderPopover, type MediaFolderPrompt } from "./MediaFolderPopover";
import { MediaDragGhost } from "./MediaDragGhost";
import { MediaPanelToolbar } from "./MediaPanelToolbar";
import { MediaPreviewCard } from "./MediaPreviewCard";
import { isTextEntryTarget } from "./isTextEntryTarget";
import { ownsEditingKeys } from "./keyboardEventOwnership";
import { useMediaFileDrop } from "./useMediaFileDrop";
import { useMediaDragGesture, type MediaDrag } from "./useMediaDragGesture";
import "./MediaPanel.css";
import { MEDIA_PANEL_PRELOAD_MARGIN, mediaPanelViewportDemand } from "./mediaPanelViewport";

export interface MediaPanelHandle {
  planCatalog(mediaItems: readonly MediaCatalogItem[], mediaUsage: readonly MediaUsage[], folders?: readonly MediaFolder[]): {
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
      thumbnailSize: number;
      onSortDirectionChange(
        mediaKind: MediaKind,
        sortDirection: MediaPanelPersistentPreference["sortDirection"],
      ): void;
      onUsageFilterChange(
        mediaKind: MediaKind,
        usageFilter: MediaPanelPersistentPreference["usageFilter"],
      ): void;
      onThumbnailSizeChange(size: number): void;
    }
  | {
      kind: "local";
      initial?: Partial<Record<MediaKind, MediaPanelPersistentPreference>>;
      initialThumbnailSize?: number;
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
  mediaFolders?: readonly MediaFolder[];
  onEditMediaFolder?(edit: MediaFolderEdit): Promise<boolean>;
  photoshopAvailable?: boolean;
  onOpenInPhotoshop?(mediaId: string): void;
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
  onReplaceMedia(mediaId: string): void;
  onRetryUnavailableMedia(mediaId: string): Promise<void>;
  relinkDisabled?: boolean;
  preferences: MediaPanelPreferenceMode;
  previewSource: MediaPanelPreviewSource;
}

const naturalNameCollator = new Intl.Collator("pt-BR", {
  numeric: true,
  sensitivity: "base",
});
const EMPTY_FOLDERS: readonly MediaFolder[] = [];
const EMPTY_MEDIA_FILES: Readonly<Record<string, MediaFileInfo>> = {};

export function MediaPanel({
  mediaFolders = EMPTY_FOLDERS,
  onEditMediaFolder,
  photoshopAvailable = false,
  onOpenInPhotoshop,
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
  onReplaceMedia,
  onRetryUnavailableMedia,
  relinkDisabled = false,
  preferences: preferenceMode,
  previewSource,
}: MediaPanelProps) {
  const mediaPreviews = previewSource.previews ?? {};
  const [folderIds, setFolderIds] = useState<Record<MediaKind, string | null>>({ photo: null, decorative: null });
  const [folderPrompt, setFolderPrompt] = useState<MediaFolderPrompt | null>(null);
  const [folderMenu, setFolderMenu] = useState<{ folder: MediaFolder; anchor: HTMLElement; x: number; y: number } | null>(null);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; mediaId: string } | null>(null);
  const onMediaDemandChange =
    previewSource.kind === "connected" ? previewSource.onDemandChange : null;
  const controlledPersistent =
    preferenceMode.kind === "controlled" ? preferenceMode.persistent : null;
  const controlledThumbnailSize =
    preferenceMode.kind === "controlled"
      ? preferenceMode.thumbnailSize
      : null;
  const [missingOnlyByKind, setMissingOnlyByKind] = useState<Record<MediaKind, boolean>>({ photo: false, decorative: false });
  const [localActiveMediaKind, setLocalActiveMediaKind] =
    useState<MediaKind>("photo");
  const activeMediaKind = preferenceMode.kind === "controlled" ? preferenceMode.activeKind : localActiveMediaKind;
  useEffect(() => { setContextMenu(null); setFolderMenu(null); setFolderPrompt(null); }, [activeMediaKind, hidden]);
  const activeFolders = useMemo(() => mediaFolders.filter((folder) => folder.kind === activeMediaKind), [mediaFolders, activeMediaKind]);
  const activeFolder = activeFolders.find((folder) => folder.id === folderIds[activeMediaKind]);
  const activeFolderMembers = useMemo(() => activeFolder ? new Set(activeFolder.mediaIds) : null, [activeFolder]);
  useEffect(() => {
    const removed = (["photo", "decorative"] as const).filter((kind) => folderIds[kind] !== null &&
      !mediaFolders.some((folder) => folder.id === folderIds[kind] && folder.kind === kind));
    if (removed.length === 0) return;
    setFolderIds((current) => ({ ...current, ...Object.fromEntries(removed.map((kind) => [kind, null])) }));
    setMissingOnlyByKind((current) => ({ ...current, ...Object.fromEntries(removed.map((kind) => [kind, false])) }));
  }, [mediaFolders, folderIds]);
  const restoreFolderFocus = useRef<HTMLElement | null>(null);
  useLayoutEffect(() => {
    if (folderPrompt || !restoreFolderFocus.current) return;
    const anchor = restoreFolderFocus.current;
    restoreFolderFocus.current = null;
    (anchor.isConnected ? anchor : panelHostRef.current)?.focus({ preventScroll: true });
  }, [folderPrompt, mediaFolders]);
  const foldersDisabled = relinkDisabled || importPending || !onEditMediaFolder;
  function closeFolderPrompt() {
    restoreFolderFocus.current = folderPrompt?.anchor ?? panelHostRef.current;
    setFolderPrompt(null);
  }
  function setActiveMediaKind(activeKind: MediaKind) {
    if (preferenceMode.kind === "controlled") preferenceMode.onActiveKindChange(activeKind);
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
  const [thumbnailSize, setThumbnailSize] = useState(() => preferenceMode.kind === "controlled"
    ? preferenceMode.thumbnailSize : preferenceMode.initialThumbnailSize ?? MEDIA_THUMBNAIL_DEFAULT_SIZE);
  const [preferencesByKind, setPreferencesByKind] = useState<
    Record<MediaKind, MediaPanelPersistentPreference>
  >(() => ({
    decorative: {
      ...createMediaPanelTabPreferences(),
      ...initialPreferences(preferenceMode, "decorative"),
    },
    photo: {
      ...createMediaPanelTabPreferences(),
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
  const search = searchByKind[activeMediaKind];
  const storedPreferences = preferencesByKind[activeMediaKind];
  const preferences = { ...storedPreferences, thumbnailSize };
  const missingOnly = folderIds[activeMediaKind] && !activeFolder ? false : missingOnlyByKind[activeMediaKind];
  const { sortKey, sortDirection, usageFilter } = preferences;
  const visibleMediaItems = useMemo(() => filterMediaItems(
    activeMediaItems.filter((media) => !activeFolderMembers || activeFolderMembers.has(media.id)), mediaUsageById, search, sortKey, sortDirection, usageFilter, fileInformation, missingOnly,
  ), [activeMediaItems, activeFolderMembers, mediaUsageById, search, sortKey, sortDirection, usageFilter, fileInformation, missingOnly]);
  const visibleMediaIds = useMemo(
    () => visibleMediaItems.map(({ id }) => id),
    [visibleMediaItems],
  );
  const visibleMediaIdSet = useMemo(
    () => new Set(visibleMediaIds),
    [visibleMediaIds],
  );
  const emptyStateReason = activeFolder?.mediaIds.length === 0 ? "folder" :
    activeMediaItems.length === 0
      ? "catalog"
      : visibleMediaItems.length === 0
        ? "filtered"
        : null;
  const gridRef = useRef<HTMLDivElement>(null);
  const panelHostRef = useRef<HTMLElement>(null);
  const [dropFolderId, setDropFolderId] = useState<string | null>(null);
  const [dragPreview, setDragPreview] = useState<MediaDrag | null>(null);
  const draggedMedia = dragPreview ? mediaItems.find((media) => media.id === dragPreview.mediaId) : undefined;
  const mediaDrag = useMediaDragGesture({ threshold: dragThreshold, disabled: Boolean(hidden) || importPending || relinkDisabled,
    onChange: (drag) => {
      setDragPreview(drag?.phase === "dragging" ? drag : null);
      const target = drag && !foldersDisabled
        ? document.elementFromPoint(drag.x, drag.y)?.closest<HTMLElement>("[data-media-folder-id]")
        : null;
      const folder = target && panelHostRef.current?.contains(target)
        ? activeFolders.find((folder) => folder.id === target.dataset.mediaFolderId && folder.kind === drag?.kind)
        : undefined;
      setDropFolderId(drag?.phase === "dragging" && folder ? folder.id : null);
      if (drag?.phase === "drop" && folder) {
        onMediaDragChange(null);
        panelHostRef.current?.focus({ preventScroll: true });
        void onEditMediaFolder?.({ kind: "moveMedia", mediaIds: [drag.mediaId], folderId: folder.id });
      } else {
        onMediaDragChange(drag);
      }
    } });
  const fileDrop = useMediaFileDrop({ port: dropPort, host: panelHostRef,
    hidden: Boolean(hidden), disabled: importPending || relinkDisabled,
    mediaKind: activeMediaKind, onImport: onImportMedia });
  const observedDemandByKind = useRef<Record<MediaKind, MediaPreviewDemand>>({
    photo: { visibleMediaIds: [], preloadMediaIds: [] },
    decorative: { visibleMediaIds: [], preloadMediaIds: [] },
  });

  useImperativeHandle(ref, () => ({
    planCatalog(nextItems, nextUsage, folders = mediaFolders) {
      const folder = folders.find((item) => item.id === activeFolder?.id);
      const members = folder ? new Set(folder.mediaIds) : null;
      const ordered = filterMediaItems(
        nextItems.filter((media) => media.kind === activeMediaKind && (!members || members.has(media.id))),
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
    if (controlledThumbnailSize !== null) setThumbnailSize(controlledThumbnailSize);
  }, [controlledThumbnailSize]);

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
    if (preferenceMode.kind === "controlled" && nextPreferences.sortKey !== undefined) {
      preferenceMode.onSortKeyChange(activeMediaKind, nextPreferences.sortKey);
    }
    if (nextPreferences.thumbnailSize !== undefined) {
      setThumbnailSize(nextPreferences.thumbnailSize);
      if (preferenceMode.kind === "controlled") preferenceMode.onThumbnailSizeChange(nextPreferences.thumbnailSize);
      const { thumbnailSize: _size, ...remaining } = nextPreferences;
      nextPreferences = remaining;
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
    if (event.defaultPrevented || folderPrompt || folderMenu || contextMenu || ownsEditingKeys(event.target)) return;
    if (matchProjectCommandShortcut(event, "media-photo") === "open-in-photoshop") {
      event.preventDefault(); event.stopPropagation();
      const selected = selectedMediaIds.size === 1 ? [...selectedMediaIds][0] : null;
      if (!event.repeat && !relinkDisabled && !importPending && photoshopAvailable && selected &&
          mediaItems.some((media) => media.id === selected && media.kind === "photo")) onOpenInPhotoshop?.(selected);
      setContextMenu(null);
      return;
    }
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
      onClickCapture={(event) => {
        if (mediaDrag.suppressClick()) { event.preventDefault(); event.stopPropagation(); }
      }}
    >
      {dragPreview && draggedMedia && !hidden && !importPending && !relinkDisabled && <MediaDragGhost
        media={draggedMedia} x={dragPreview.x} y={dragPreview.y}
        previewUrl={mediaPreviews[draggedMedia.id]?.url ?? undefined}
        missing={fileInformation[draggedMedia.id]?.state === "absent"} />}
      {fileDrop.over && <div className="media-file-drop-hint" role="status">Solte para importar em {activeMediaKind === "photo" ? "Fotos" : "Decorativos"}</div>}
      {fileDrop.error && <div className="media-file-drop-error" role="status">{fileDrop.error}</div>}
      <MediaPanelToolbar
        activeMediaKind={activeMediaKind}
        folders={activeFolders}
        activeFolderId={activeFolder?.id ?? null}
        dropFolderId={dropFolderId}
        foldersDisabled={foldersDisabled}
        onFolderChange={(id) => { setContextMenu(null); setFolderIds((current) => ({ ...current, [activeMediaKind]: id })); }}
        onCreateFolder={(anchor) => { setContextMenu(null); setFolderPrompt({ kind: "create", mediaKind: activeMediaKind, anchor }); }}
        onFolderMenu={(folder, anchor, position) => { setContextMenu(null); setFolderMenu({ folder, anchor, ...position }); }}
        missingCounts={missingCounts}
        missingOnly={missingOnly}
        onMissingOnlyChange={(value) => {
          setMissingOnlyByKind((current) => ({ ...current, [activeMediaKind]: value }));
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
          setSearchByKind((current) => ({
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
                missing={(file?.state ?? preview?.state) === "absent"}
                selected={isSelected}
                onClick={(event) => { if (!mediaDrag.suppressClick()) selectMedia(media.id, event); }}
                onContextMenu={(event) => {
                  event.preventDefault();
                  selectMediaForContextMenu(media.id);
                  setContextMenu({ x: event.clientX, y: event.clientY, mediaId: media.id });
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
                {availabilityLabel && (
                  <span
                    aria-label={availabilityLabel ?? undefined}
                    className={preview?.state === "absent" ? "media-missing-indicator" : "media-availability"}
                    role="status"
                    title={availabilityLabel}
                  >
                    {preview?.state === "absent" ? <AppIcon icon={ImageOff} size={16} /> : availabilityLabel}
                  </span>
                )}
                </MediaPreviewCard>
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
        {fileInformation[contextMenu.mediaId]?.state === "absent" && (
          <button type="button" role="menuitem" disabled={relinkDisabled || importPending}
            onClick={() => {
              const mediaId = contextMenu.mediaId;
              setContextMenu(null);
              panelHostRef.current?.focus({ preventScroll: true });
              onRelinkMedia(mediaId);
            }}>
            {projectCommandDescriptor("relink-media").label}
          </button>
        )}
        <button type="button" role="menuitem" disabled={relinkDisabled || importPending}
          onClick={() => {
            const mediaId = contextMenu.mediaId;
            setContextMenu(null);
            panelHostRef.current?.focus({ preventScroll: true });
            onReplaceMedia(mediaId);
          }}>
          {projectCommandDescriptor("replace-media").label}
        </button>
        {mediaItems.some((media) => selectedMediaIds.has(media.id) && media.kind === "photo") && <button type="button" role="menuitem"
          disabled={!photoshopAvailable || relinkDisabled || importPending || selectedMediaIds.size !== 1}
          onClick={() => { const id = [...selectedMediaIds][0]; if (id) onOpenInPhotoshop?.(id); setContextMenu(null); panelHostRef.current?.focus({ preventScroll: true }); }}>
          <span>{projectCommandDescriptor("open-in-photoshop").label}</span><kbd aria-hidden="true">{projectCommandShortcutLabel("open-in-photoshop")}</kbd>
        </button>}
        {onEditMediaFolder && <button type="button" role="menuitem" disabled={foldersDisabled || selectedMediaIds.size === 0}
          onClick={() => {
            const anchor = panelHostRef.current?.querySelector<HTMLElement>(`[data-media-id="${contextMenu.mediaId}"]`) ?? panelHostRef.current;
            const currentFolder = activeFolders.find((folder) => folder.mediaIds.includes(contextMenu.mediaId));
            if (anchor) setFolderPrompt({ kind: "move", mediaKind: activeMediaKind, anchor,
              mediaIds: [...selectedMediaIds], folderId: currentFolder?.id ?? activeFolders[0]?.id ?? null });
            setContextMenu(null);
          }}>{projectCommandDescriptor("move-media-to-folder").label}</button>}
        <button type="button" role="menuitem" disabled={relinkDisabled || importPending || selectedMediaIds.size === 0}
          onClick={() => { setContextMenu(null); onRemoveMedia([...selectedMediaIds]); panelHostRef.current?.focus({ preventScroll: true }); }}>
          <span>{projectCommandDescriptor("remove-media").label}</span><kbd aria-hidden="true">{projectCommandShortcutLabel("remove-media")}</kbd>
        </button>
      </ContextMenuSurface>}
      {folderMenu && <ContextMenuSurface label={`Ações da pasta ${folderMenu.folder.name}`} position={folderMenu}
        onDismiss={() => { folderMenu.anchor.focus({ preventScroll: true }); setFolderMenu(null); }}>
        <button type="button" role="menuitem" disabled={foldersDisabled} onClick={() => {
          setFolderPrompt({ kind: "rename", folder: folderMenu.folder, anchor: folderMenu.anchor, mediaKind: activeMediaKind });
          setFolderMenu(null);
        }}>{projectCommandDescriptor("rename-media-folder").label}</button>
        <button type="button" role="menuitem" disabled={foldersDisabled} onClick={() => {
          void onEditMediaFolder?.({ kind: "delete", folderId: folderMenu.folder.id });
          setFolderMenu(null); panelHostRef.current?.focus({ preventScroll: true });
        }}>{projectCommandDescriptor("delete-media-folder").label}</button>
      </ContextMenuSurface>}
      {folderPrompt && onEditMediaFolder && <MediaFolderPopover prompt={folderPrompt} folders={mediaFolders}
        onSubmit={(edit) => foldersDisabled ? Promise.resolve(false) : onEditMediaFolder(edit)} onClose={closeFolderPrompt} />}
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
    case "cache_paused":
      return `Prévia aguardando espaço${previous}`;
    case "ready":
      return null;
  }
}

function initialPreferences(
  mode: MediaPanelPreferenceMode,
  mediaKind: MediaKind,
): Partial<MediaPanelPersistentPreference> {
  return mode.kind === "controlled"
    ? {
        ...mode.persistent[mediaKind],
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
