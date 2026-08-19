import { useEffect, useRef, useState } from "react";
import {
  ArrowDownAZ,
  ArrowUpAZ,
  ChevronUp,
  Image as ImageIcon,
  Layers3,
  Search,
  X,
} from "lucide-react";

import type { MediaKind } from "../domain/project";
import {
  MEDIA_THUMBNAIL_DEFAULT_SIZE,
  MEDIA_THUMBNAIL_MAX_SIZE,
  MEDIA_THUMBNAIL_MIN_SIZE,
  type MediaPanelViewPreferences,
  type MediaUsageFilter,
} from "../state/mediaPanelPreferences";
import { AppIcon } from "../ui";

interface MediaPanelToolbarProps {
  activeMediaKind: MediaKind;
  itemCount: number;
  onActiveMediaKindChange(mediaKind: MediaKind): void;
  onPreferencesChange(preferences: Partial<MediaPanelViewPreferences>): void;
  onSearchChange(search: string): void;
  preferences: MediaPanelViewPreferences;
  search: string;
}

const PLACEHOLDER_TITLE = "Ainda não disponível nesta versão";

export function MediaPanelToolbar({
  activeMediaKind,
  itemCount,
  onActiveMediaKindChange,
  onPreferencesChange,
  onSearchChange,
  preferences,
  search,
}: MediaPanelToolbarProps) {
  const [importMenuOpen, setImportMenuOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const activeKindLabel =
    activeMediaKind === "photo" ? "Fotos" : "Decorativos";

  useEffect(() => {
    if (!importMenuOpen) return;

    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        !rootRef.current?.contains(event.target)
      ) {
        setImportMenuOpen(false);
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setImportMenuOpen(false);
    };

    document.addEventListener("pointerdown", closeOnOutsidePointer);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsidePointer);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [importMenuOpen]);

  function changeMediaKind(mediaKind: MediaKind) {
    setImportMenuOpen(false);
    onActiveMediaKindChange(mediaKind);
  }

  return (
    <div className="media-toolbar" ref={rootRef}>
      <div aria-label="Tipo de recurso" className="media-tabs" role="group">
        <button
          aria-label="Fotos"
          aria-pressed={activeMediaKind === "photo"}
          className={activeMediaKind === "photo" ? "active" : undefined}
          title="Fotos"
          type="button"
          onClick={() => changeMediaKind("photo")}
        >
          <AppIcon icon={ImageIcon} size={16} />
        </button>
        <button
          aria-label="Decorativos"
          aria-pressed={activeMediaKind === "decorative"}
          className={activeMediaKind === "decorative" ? "active" : undefined}
          title="Decorativos"
          type="button"
          onClick={() => changeMediaKind("decorative")}
        >
          <AppIcon icon={Layers3} size={16} />
        </button>
      </div>

      <div className="media-import-menu">
        <button
          aria-expanded={importMenuOpen}
          aria-haspopup="menu"
          className="media-toolbar-text-button"
          type="button"
          onClick={() => setImportMenuOpen((open) => !open)}
        >
          <span>Importar</span>
          <AppIcon icon={ChevronUp} size={12} />
        </button>
        {importMenuOpen && (
          <div aria-label="Importar" className="media-popup media-import-popup" role="menu">
            {/* PLACEHOLDER UI: import commands await their application port. */}
            <button
              data-placeholder-feature="import-media-files"
              disabled
              role="menuitem"
              title={PLACEHOLDER_TITLE}
              type="button"
            >
              Arquivos…
            </button>
            <button
              data-placeholder-feature="import-media-folder"
              disabled
              role="menuitem"
              title={PLACEHOLDER_TITLE}
              type="button"
            >
              Pasta…
            </button>
          </div>
        )}
      </div>

      <label className="media-toolbar-filter">
        <span className="ui-visually-hidden">Filtro de uso</span>
        <select
          aria-label="Filtro de uso"
          value={preferences.usageFilter}
          onChange={(event) =>
            onPreferencesChange({
              usageFilter: event.target.value as MediaUsageFilter,
            })
          }
        >
          <option value="all">Todas</option>
          <option value="used">Usadas</option>
          <option value="unused">Não usadas</option>
        </select>
        <small>{itemCount}</small>
      </label>

      <div className="media-toolbar-spacer" />

      <div
        aria-label={`Busca em ${activeKindLabel}`}
        className="media-search"
        role="search"
      >
        <AppIcon icon={Search} size={12} />
        <input
          aria-label={`Buscar ${activeKindLabel}`}
          placeholder="Buscar…"
          role="searchbox"
          value={search}
          onChange={(event) => onSearchChange(event.target.value)}
        />
        {search && (
          <button
            aria-label="Limpar busca"
            type="button"
            onClick={() => onSearchChange("")}
          >
            <AppIcon icon={X} size={12} />
          </button>
        )}
      </div>

      <label className="media-toolbar-sort">
        <span>Ordenar:</span>
        <select aria-label="Ordenar por" value="name" onChange={() => undefined}>
          <option value="name">Nome</option>
          {/* PLACEHOLDER UI: MediaCatalogItem has no date metadata yet. */}
          <option
            data-placeholder-feature="sort-media-by-created-at"
            disabled
            value="created-at"
          >
            Data de criação
          </option>
          <option
            data-placeholder-feature="sort-media-by-modified-at"
            disabled
            value="modified-at"
          >
            Data de alteração
          </option>
        </select>
      </label>

      <button
        aria-label={
          preferences.sortDirection === "ascending"
            ? "Ordem crescente"
            : "Ordem decrescente"
        }
        className="media-sort-direction"
        title={
          preferences.sortDirection === "ascending"
            ? "Ordem crescente"
            : "Ordem decrescente"
        }
        type="button"
        onClick={() =>
          onPreferencesChange({
            sortDirection:
              preferences.sortDirection === "ascending"
                ? "descending"
                : "ascending",
          })
        }
      >
        <AppIcon
          icon={
            preferences.sortDirection === "ascending"
              ? ArrowDownAZ
              : ArrowUpAZ
          }
          size={14}
        />
      </button>

      <label className="media-toolbar-size">
        <span className="ui-visually-hidden">Tamanho das miniaturas</span>
        <input
          aria-label="Tamanho das miniaturas"
          max={MEDIA_THUMBNAIL_MAX_SIZE}
          min={MEDIA_THUMBNAIL_MIN_SIZE}
          step="2"
          title={`Tamanho das miniaturas: ${preferences.thumbnailSize} px`}
          type="range"
          value={preferences.thumbnailSize}
          onChange={(event) =>
            onPreferencesChange({
              thumbnailSize: Number(event.target.value),
            })
          }
          onDoubleClick={() =>
            onPreferencesChange({
              thumbnailSize: MEDIA_THUMBNAIL_DEFAULT_SIZE,
            })
          }
        />
      </label>
    </div>
  );
}
