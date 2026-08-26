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
import { MediaPreviewCard } from "./MediaPreviewCard";
import "./DecorativeMediaPicker.css";
import "./VisualDefaultPicker.css";

const IMPORT_PLACEHOLDER_TITLE = "Ainda não disponível nesta versão";

interface DecorativeMediaPickerProps {
  decorativeMedia: readonly MediaCatalogItem[];
  label: "Background" | "Overlay";
  mediaPreviewUrls: Readonly<Record<string, string>>;
  open: boolean;
  onOpenChange(open: boolean): void;
  onSelect(mediaId: string): void;
  selectedMediaId: string | null;
}

export function DecorativeMediaPicker({
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
    const selected = menu?.querySelector<HTMLElement>(
      '[role="menuitem"][data-selected="true"]',
    );
    const first = menu?.querySelector<HTMLElement>(
      '[role="menuitem"]:not(:disabled)',
    );
    (selected ?? first ?? menu)?.focus({ preventScroll: true });
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
        const openingAnotherPicker =
          event.target instanceof Element &&
          event.target.closest("[data-decorative-picker-trigger]");
        if (!openingAnotherPicker) window.setTimeout(restoreTrigger, 0);
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
    const menuItems = Array.from(
      event.currentTarget.querySelectorAll<HTMLElement>(
        '[role="menuitem"]:not(:disabled)',
      ),
    );
    if (menuItems.length === 0) return;
    const currentIndex = menuItems.indexOf(
      document.activeElement as HTMLElement,
    );
    let nextIndex: number | null = null;
    if (event.key === "Home") nextIndex = 0;
    if (event.key === "End") nextIndex = menuItems.length - 1;
    if (["ArrowDown", "ArrowRight"].includes(event.key)) {
      nextIndex = (currentIndex + 1) % menuItems.length;
    }
    if (["ArrowUp", "ArrowLeft"].includes(event.key)) {
      nextIndex =
        (currentIndex <= 0 ? menuItems.length : currentIndex) - 1;
    }
    if (event.key === "Tab") {
      onOpenChange(false);
      return;
    }
    if (nextIndex === null) return;
    event.preventDefault();
    menuItems[nextIndex]?.focus({ preventScroll: true });
  }

  return (
    <div className="visual-default-decorative" ref={rootRef}>
      <button
        aria-expanded={open}
        aria-haspopup="menu"
        aria-label={
          applied
            ? `Decorativo do ${label}: ${applied.name}. Escolher outro`
            : `Escolher Decorativo para ${label}`
        }
        className="visual-default-picker__option"
        data-decorative-picker-trigger="true"
        data-selected={applied ? true : undefined}
        ref={triggerRef}
        title={applied ? applied.name : "Escolher Decorativo"}
        type="button"
        onClick={() => onOpenChange(!open)}
      >
        {applied ? (
          <span
            aria-hidden="true"
            className="visual-default-picker__tile"
            style={decorativePreview(applied, mediaPreviewUrls)}
          />
        ) : (
          <span
            aria-hidden="true"
            className="visual-default-picker__tile visual-default-picker__tile--add"
          >
            <AppIcon icon={Plus} size={12} />
          </span>
        )}
      </button>
      {open ? (
        <div
          aria-label={`Decorativos para ${label}`}
          className="ui-floating-surface visual-default-popup"
          ref={menuRef}
          role="menu"
          tabIndex={-1}
          onKeyDown={navigateMenu}
        >
          <div className="visual-default-popup__grid" role="none">
            {decorativeMedia.map((media) => {
              const selected = selectedMediaId === media.id;
              return (
                <MediaPreviewCard
                  aria-label={`Usar ${label} ${media.name}${
                    selected ? ". Selecionado" : ""
                  }`}
                  key={media.id}
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
              aria-label="Importar Decorativo"
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
