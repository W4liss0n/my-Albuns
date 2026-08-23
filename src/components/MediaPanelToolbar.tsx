import { useEffect, useRef, useState } from "react";
import {
  ChevronDown,
  Image as ImageIcon,
  ListFilter,
  Plus,
  SlidersHorizontal,
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
type OpenPopup = "import" | "options" | null;

export function MediaPanelToolbar({
  activeMediaKind,
  itemCount,
  onActiveMediaKindChange,
  onPreferencesChange,
  onSearchChange,
  preferences,
  search,
}: MediaPanelToolbarProps) {
  const [openPopup, setOpenPopup] = useState<OpenPopup>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const optionsButtonRef = useRef<HTMLButtonElement>(null);
  const activeKindLabel =
    activeMediaKind === "photo" ? "Fotos" : "Decorativos";

  useEffect(() => {
    if (!openPopup) return;

    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        !rootRef.current?.contains(event.target)
      ) {
        setOpenPopup(null);
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      const restoreOptionsFocus = openPopup === "options";
      setOpenPopup(null);
      if (restoreOptionsFocus) optionsButtonRef.current?.focus();
    };

    document.addEventListener("pointerdown", closeOnOutsidePointer);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsidePointer);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [openPopup]);

  function changeMediaKind(mediaKind: MediaKind) {
    setOpenPopup(null);
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
          <AppIcon icon={SlidersHorizontal} size={14} />
        </button>
      </div>

      <div className="media-import-menu">
        <button
          aria-expanded={openPopup === "import"}
          aria-haspopup="menu"
          className="media-toolbar-text-button"
          type="button"
          onClick={() =>
            setOpenPopup((current) =>
              current === "import" ? null : "import",
            )
          }
        >
          <span>Importar</span>
          <AppIcon icon={ChevronDown} size={12} />
        </button>
        {openPopup === "import" && (
          <div
            aria-label="Importar"
            className="media-popup media-import-popup"
            role="menu"
          >
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

      <div className="media-folder-bar">
        <div className="media-folder-strip">
          <button
            aria-label={`Todas ${itemCount}`}
            aria-pressed="true"
            className="media-folder-chip active"
            type="button"
          >
            <span>Todas</span>
            <small>{itemCount}</small>
          </button>
          {/*
            PLACEHOLDER UI: organization chips belong here after the Project
            exposes Media organization folders through an application port.
          */}
        </div>
        <button
          aria-label="Nova pasta de organização"
          className="media-folder-add"
          data-placeholder-feature="media-organization-folders"
          disabled
          title={PLACEHOLDER_TITLE}
          type="button"
        >
          <AppIcon icon={Plus} size={12} />
        </button>
      </div>

      <div className="media-toolbar-actions">
        <div
          aria-label={`Busca em ${activeKindLabel}`}
          className="media-search ui-embedded-field"
          role="search"
        >
          <input
            aria-label={`Buscar ${activeKindLabel}`}
            className="ui-embedded-input"
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

        <button
          aria-controls="media-panel-options"
          aria-expanded={openPopup === "options"}
          aria-label="Filtro, ordem e tamanho"
          className={`media-options-button${openPopup === "options" ? " active" : ""}`}
          ref={optionsButtonRef}
          title="Filtro, ordem e tamanho"
          type="button"
          onClick={() =>
            setOpenPopup((current) =>
              current === "options" ? null : "options",
            )
          }
        >
          <AppIcon icon={ListFilter} size={14} />
        </button>

        {openPopup === "options" && (
          <div
            aria-label="Filtro, ordem e tamanho"
            className="media-popup media-options-popup"
            id="media-panel-options"
            role="group"
          >
            <label className="media-options-row">
              <span>Filtro</span>
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
            </label>

            <label className="media-options-row">
              <span>Ordem</span>
              <select
                aria-label="Ordenar por"
                value={`name-${preferences.sortDirection}`}
                onChange={(event) => {
                  if (event.target.value === "name-ascending") {
                    onPreferencesChange({ sortDirection: "ascending" });
                  }
                  if (event.target.value === "name-descending") {
                    onPreferencesChange({ sortDirection: "descending" });
                  }
                }}
              >
                <option value="name-ascending">Nome</option>
                <option value="name-descending">Nome (inverso)</option>
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

            <label className="media-options-row media-options-size">
              <span>Tamanho</span>
              <input
                aria-label="Tamanho das miniaturas"
                className="ui-range"
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
        )}
      </div>
    </div>
  );
}
