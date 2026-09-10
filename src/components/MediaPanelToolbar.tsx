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
import { AppIcon, TextInput } from "../ui";
import { useDismissableSurface } from "../ui/useDismissableSurface";

interface MediaPanelToolbarProps {
  activeMediaKind: MediaKind;
  missingCounts: Readonly<Record<MediaKind, number>>;
  missingOnly: boolean;
  reviewingMissing: boolean;
  onMissingOnlyChange(value: boolean): void;
  importDisabled?: boolean;
  itemCount: number;
  importPending?: boolean;
  onImportPhoto(): void;
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
  missingCounts,
  missingOnly,
  reviewingMissing,
  onMissingOnlyChange,
  importDisabled = false,
  itemCount,
  importPending = false,
  onImportPhoto,
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

  useDismissableSurface({
    enabled: openPopup !== null,
    rootRef,
    onDismiss: ({ reason, event }) => {
      if (reason === "pointerOutside") {
        setOpenPopup(null);
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      const restoreOptionsFocus = openPopup === "options";
      setOpenPopup(null);
      if (restoreOptionsFocus) optionsButtonRef.current?.focus();
    },
  });

  useEffect(() => {
    if (importDisabled && openPopup === "import") setOpenPopup(null);
  }, [importDisabled, openPopup]);
  useEffect(() => { setOpenPopup(null); }, [activeMediaKind, reviewingMissing]);

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
          {missingCounts.photo > 0 && <span className="media-missing-badge" aria-label={`${missingCounts.photo} Fotos com arquivo ausente`}>{missingCounts.photo}</span>}
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
          {missingCounts.decorative > 0 && <span className="media-missing-badge" aria-label={`${missingCounts.decorative} Decorativos com arquivo ausente`}>{missingCounts.decorative}</span>}
        </button>
      </div>

      <div className="media-import-menu">
        <button
          aria-expanded={openPopup === "import"}
          aria-haspopup="menu"
          className="media-toolbar-text-button"
          disabled={importDisabled}
          type="button"
          onClick={() =>
            setOpenPopup((current) =>
              current === "import" ? null : "import",
            )
          }
        >
          <span>{importPending ? "Processando…" : "Importar"}</span>
          <AppIcon icon={ChevronDown} size={12} />
        </button>
        {openPopup === "import" && (
          <div
            aria-label="Importar"
            className="ui-floating-surface media-popup media-import-popup"
            role="menu"
          >
            <button
              disabled={activeMediaKind !== "photo" || importDisabled}
              role="menuitem"
              type="button"
              onClick={() => {
                setOpenPopup(null);
                onImportPhoto();
              }}
            >
              Arquivos JPEG…
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
            aria-pressed={!missingOnly}
            className={`media-folder-chip${!missingOnly ? " active" : ""}`}
            type="button"
            onClick={() => onMissingOnlyChange(false)}
          >
            <span>Todas</span>
            <small>{itemCount}</small>
          </button>
          <button type="button" className={`media-folder-chip${missingOnly ? " active" : ""}`}
            aria-pressed={missingOnly} onClick={() => onMissingOnlyChange(!missingOnly)}>
            Ausentes<small>{missingCounts[activeMediaKind]}</small>
          </button>
          {reviewingMissing && <button type="button" className="media-folder-add"
            aria-label="Encerrar visualização de ausentes" title="Voltar à aba e aos filtros anteriores"
            onClick={() => onMissingOnlyChange(false)}><AppIcon icon={X} size={12} /></button>}
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
          <TextInput
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
            className="ui-floating-surface media-popup media-options-popup"
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
                value={`${preferences.sortKey}-${preferences.sortDirection}`}
                onChange={(event) => {
                  const [sortKey, sortDirection] = event.target.value.split("-");
                  onPreferencesChange({ sortKey: sortKey as MediaPanelViewPreferences["sortKey"],
                    sortDirection: sortDirection as MediaPanelViewPreferences["sortDirection"] });
                }}
              >
                <option value="name-ascending">Nome</option>
                <option value="name-descending">Nome (inverso)</option>
                <option value="createdAt-ascending">Data de criação</option>
                <option value="createdAt-descending">Data de criação (inversa)</option>
                <option value="modifiedAt-ascending">Data de alteração</option>
                <option value="modifiedAt-descending">Data de alteração (inversa)</option>
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
