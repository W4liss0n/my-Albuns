import {
  useEffect,
  useRef,
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { Plus } from "lucide-react";

import type { MediaCatalogItem } from "../domain/project";
import { AppIcon } from "../ui";
import { useDismissableSurface } from "../ui/useDismissableSurface";
import { focusMenuItem } from "../ui/menuNavigation";
import { MediaPreviewCard } from "./MediaPreviewCard";
import "./DecorativeMediaPicker.css";
import "./VisualDesignControl.css";

const IMPORT_PLACEHOLDER_TITLE = "Ainda não disponível nesta versão";

interface DecorativeMediaPickerProps {
  disabled?: boolean;
  decorativeMedia: readonly MediaCatalogItem[];
  label: "Fundo" | "Sobreposição";
  mediaPreviewUrls: Readonly<Record<string, string>>;
  open: boolean;
  onOpenChange(open: boolean): void;
  onSelect(mediaId: string): void;
  selectedMediaId: string | null;
}

export function DecorativeMediaPicker({
  disabled = false,
  decorativeMedia,
  label,
  mediaPreviewUrls,
  open,
  onOpenChange,
  onSelect,
  selectedMediaId,
}: DecorativeMediaPickerProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const applied =
    decorativeMedia.find((media) => media.id === selectedMediaId) ?? null;

  useEffect(() => {
    if (!open) return;
    const menu = menuRef.current;
    if (menu) focusMenuItem(menu, "selected", { preventScroll: true });
  }, [open]);

  useDismissableSurface({
    enabled: open,
    rootRef,
    onDismiss: ({ reason, event }) => {
      const restoreTrigger = () => {
        triggerRef.current?.focus({ preventScroll: true });
      };
      if (reason === "pointerOutside") {
        onOpenChange(false);
        window.setTimeout(() => {
          if (document.activeElement === document.body) restoreTrigger();
        }, 0);
        return;
      }
      event.preventDefault();
      onOpenChange(false);
      restoreTrigger();
    },
  });

  function closeAndRestoreFocus() {
    onOpenChange(false);
    triggerRef.current?.focus({ preventScroll: true });
  }

  function navigateMenu(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key === "Tab") {
      onOpenChange(false);
      return;
    }
    const target = event.key === "Home" ? "first"
      : event.key === "End" ? "last"
      : ["ArrowDown", "ArrowRight"].includes(event.key) ? "next"
      : ["ArrowUp", "ArrowLeft"].includes(event.key) ? "previous"
      : null;
    if (target === null) return;
    event.preventDefault();
    focusMenuItem(event.currentTarget, target, { preventScroll: true, fallbackToContainer: false });
  }

  return (
    <div className="visual-design-decorative" ref={rootRef}>
      <button
        aria-expanded={open}
        aria-haspopup="menu"
        aria-label={
          applied
            ? `Decorativo ${label === "Fundo" ? "do fundo" : "da sobreposição"}: ${applied.name}. Escolher outro`
            : `Escolher decorativo para ${label.toLocaleLowerCase("pt-BR")}`
        }
        className="visual-design-picker__option"
        data-decorative-picker-trigger="true"
        disabled={disabled}
        data-selected={applied ? true : undefined}
        ref={triggerRef}
        title={applied ? applied.name : "Escolher decorativo"}
        type="button"
        onClick={() => onOpenChange(!open)}
      >
        {applied ? (
          <span
            aria-hidden="true"
            className="visual-design-picker__tile"
            style={decorativePreview(applied, mediaPreviewUrls)}
          />
        ) : (
          <span
            aria-hidden="true"
            className="visual-design-picker__tile visual-design-picker__tile--add"
          >
            <AppIcon icon={Plus} size={12} />
          </span>
        )}
      </button>
      {open ? (
        <div
          aria-label={`Decorativos para ${label.toLocaleLowerCase("pt-BR")}`}
          className="ui-floating-surface visual-design-popup"
          ref={menuRef}
          role="menu"
          tabIndex={-1}
          onKeyDown={navigateMenu}
        >
          <div className="visual-design-popup__grid" role="none">
            {decorativeMedia.map((media) => {
              const selected = selectedMediaId === media.id;
              return (
                <MediaPreviewCard
                  aria-label={`Usar ${label.toLocaleLowerCase("pt-BR")} ${media.name}${
                    selected ? ". Selecionado" : ""
                  }`}
                  key={media.id}
                  disabled={disabled}
                  kind="media"
                  loading="eager"
                  media={media}
                  previewUrl={mediaPreviewUrls[media.id]}
                  role="menuitem"
                  selected={selected}
                  title={media.name}
                  onClick={() => {
                    onSelect(media.id);
                    closeAndRestoreFocus();
                  }}
                />
              );
            })}
            {/* PLACEHOLDER UI: import commands await their application port. */}
            <MediaPreviewCard
              aria-label="Importar decorativo"
              data-placeholder-feature="import-decorative-files"
              disabled
              kind="placeholder"
              role="menuitem"
              title={IMPORT_PLACEHOLDER_TITLE}
            >
              <AppIcon icon={Plus} size={16} />
            </MediaPreviewCard>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function decorativePreview(
  media: MediaCatalogItem,
  mediaPreviewUrls: Readonly<Record<string, string>>,
): CSSProperties {
  const url = mediaPreviewUrls[media.id];
  return url
    ? {
        backgroundImage: `url("${url}")`,
        backgroundPosition: "center",
        backgroundRepeat: "no-repeat",
        backgroundSize: "contain",
      }
    : { background: media.palette?.[1] ?? "var(--ui-surface-muted)" };
}
