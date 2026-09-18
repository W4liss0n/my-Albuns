import { useId, type ReactNode } from "react";
import type { MediaCatalogItem } from "../domain/project";
import { DecorativeMediaPicker } from "./DecorativeMediaPicker";
import "./VisualDesignControl.css";

export function VisualDesignControl({
  children,
  actions,
  description,
  disabled = false,
  decorativeMedia,
  label,
  mediaPreviewUrls,
  noneSelected = false,
  open,
  onClear,
  onOpenChange,
  onSelect,
  selectedMediaId,
}: {
  children?: ReactNode;
  actions?: ReactNode;
  description?: string;
  disabled?: boolean;
  decorativeMedia: readonly MediaCatalogItem[];
  label: "Fundo" | "Sobreposição";
  mediaPreviewUrls: Readonly<Record<string, string>>;
  /**
   * Verdadeiro apenas quando o escopo inteiro está sem sobreposição. Escopo com
   * lados divergentes não é ausência, e não deve marcar `Sem sobreposição`.
   */
  noneSelected?: boolean;
  open: boolean;
  onClear?: () => void;
  onOpenChange(open: boolean): void;
  onSelect(mediaId: string): void;
  selectedMediaId: string | null;
}) {
  const descriptionId = useId();
  return (
    <div className="visual-design-field">
      <span className="visual-design-label" title={description}>{label}</span>
      <div
        aria-describedby={description ? descriptionId : undefined}
        aria-label={`Opções de ${label.toLocaleLowerCase("pt-BR")}`}
        className="visual-design-picker"
        role="group"
      >
        {children}
        {onClear ? (
          <button
            aria-label="Sem sobreposição"
            aria-pressed={noneSelected}
            disabled={disabled}
            className="visual-design-picker__option"
            title="Sem sobreposição"
            type="button"
            onClick={onClear}
          >
            <span
              aria-hidden="true"
              className="visual-design-picker__tile visual-design-picker__preview--none"
            />
          </button>
        ) : null}
        <span aria-hidden="true" className="visual-design-picker__divider" />
        <DecorativeMediaPicker
          disabled={disabled}
          decorativeMedia={decorativeMedia}
          label={label}
          mediaPreviewUrls={mediaPreviewUrls}
          open={open && !disabled}
          selectedMediaId={selectedMediaId}
          onOpenChange={onOpenChange}
          onSelect={onSelect}
        />
      </div>
      {actions ? <div className="visual-design-actions">{actions}</div> : null}
      {description ? <span className="ui-visually-hidden" id={descriptionId}>{description}</span> : null}
    </div>
  );
}
