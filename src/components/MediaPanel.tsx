import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { ImageOff, Sparkles } from "lucide-react";

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
import { AppIcon, EmptyState } from "../ui";
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
        itemCount={visibleMediaItems.length}
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
          <EmptyState
            className="media-empty-state"
            density="compact"
            description={
              activeMediaKind === "photo"
                ? "As Fotos importadas para este Projeto aparecerão aqui."
                : "As Imagens decorativas importadas aparecerão aqui."
            }
            icon={
              <AppIcon
                icon={activeMediaKind === "photo" ? ImageOff : Sparkles}
                size={16}
              />
            }
            title={
              activeMediaKind === "photo"
                ? "Nenhuma Foto importada"
                : "Nenhum Decorativo importado"
            }
          />
        ) : visibleMediaItems.length === 0 ? (
          <EmptyState
            className="media-empty-state"
            density="compact"
            description="Ajuste a busca ou o filtro de uso para ver outros itens."
            icon={<AppIcon icon={ImageOff} size={16} />}
            title="Nenhum item encontrado"
          />
        ) : (
          visibleMediaItems.map((media) => (
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
          ))
        )}
      </div>
    </section>
  );
}

function mediaCardBackground(media: MediaCatalogItem) {
  const palette = media.palette ?? ["#26323A", "#53636D", "#A6B0B6"];
  return `linear-gradient(135deg, ${palette[0]}, ${palette[1]} 56%, ${palette[2]})`;
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
