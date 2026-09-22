import { useId, type ReactNode } from "react";
import type { MediaCatalogItem } from "../domain/project";
import { DecorativeMediaPicker } from "./DecorativeMediaPicker";
import { ColorPropertyControl } from "../ui/ColorPropertyControl";
import { summarizeVisualSelection, type VisualSelectionValue } from "../ui/visualSelection";
import "./VisualDesignControl.css";

export function VisualDesignControl({
  color,
  actions,
  description,
  disabled = false,
  decorativeMedia,
  label,
  mediaPreviewUrls,
  open,
  onClear,
  onOpenChange,
  onSelect,
  values,
}: {
  color?: { label: string; onCommit(rgb: string): void };
  actions?: ReactNode;
  description?: string;
  disabled?: boolean;
  decorativeMedia: readonly MediaCatalogItem[];
  label: "Fundo" | "Sobreposição";
  mediaPreviewUrls: Readonly<Record<string, string>>;
  open: boolean;
  onClear?: () => void;
  onOpenChange(open: boolean): void;
  onSelect(mediaId: string): void;
  values: readonly VisualSelectionValue[];
}) {
  const descriptionId = useId();
  const selection = summarizeVisualSelection(values);
  const accessibleDescription = [selection.mixed ? "Valores diferentes." : "", description].filter(Boolean).join(" ");
  return (
    <div className="visual-design-field">
      <span className="visual-design-label" title={accessibleDescription}>{label}</span>
      <div
        aria-describedby={accessibleDescription ? descriptionId : undefined}
        aria-label={`Opções de ${label.toLocaleLowerCase("pt-BR")}`}
        className="visual-design-picker"
        role="group"
      >
        {color && <div className="visual-design-color-property">
          <ColorPropertyControl label={color.label} defaultRgb="#FFFFFF" disabled={disabled}
            rgb={selection.rgb} mixed={selection.mixed} previewColors={selection.previewColors}
            description={selection.mixed ? accessibleDescription : undefined} onCommit={color.onCommit} />
        </div>}
        {onClear ? (
          <button
            aria-label="Sem sobreposição"
            aria-pressed={selection.none}
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
          selectedMediaId={selection.mediaId}
          mixed={selection.mixed && values.some(value => value.kind === "media")}
          onOpenChange={onOpenChange}
          onSelect={onSelect}
        />
      </div>
      {actions ? <div className="visual-design-actions">{actions}</div> : null}
      {accessibleDescription ? <span className="ui-visually-hidden" id={descriptionId}>{accessibleDescription}</span> : null}
    </div>
  );
}
