import { useRef, useState, type CSSProperties, type ReactNode } from "react";
import { RotateCcw, X } from "lucide-react";

import type { VisualScope } from "../application/scopedValues";
import type {
  ComposedSheet, DecorativeRole, DecorativeScope, MediaCatalogItem, SheetVisualChange, SheetVisuals,
} from "../domain/project";
import { ActionButton, AppIcon } from "../ui";
import { VisualScopeControls } from "../ui/visualPreview/VisualScopeControls";
import { SheetPreview } from "./SheetPreview";
import { VisualDesignControl } from "./VisualDesignControl";
import "./SheetDesignInspector.css";

export type SheetDesignScope = VisualScope;

export interface SheetDesignActions {
  disabled: boolean;
  onChange(sheetId: string, scope: DecorativeScope, change: SheetVisualChange): Promise<boolean>;
  onApplyDecorative(sheetId: string, scope: DecorativeScope, role: DecorativeRole, mediaId: string): Promise<boolean>;
}

interface SheetDesignInspectorProps {
  actions?: SheetDesignActions;
  visuals?: SheetVisuals;
  saveLayout?: { enabled: boolean; onSave(): void; feedback?: ReactNode };
  mediaItems?: readonly MediaCatalogItem[];
  mediaPreviewUrls: Readonly<Record<string, string>>;
  scope: SheetDesignScope;
  sheet: ComposedSheet;
  onScopeChange(scope: SheetDesignScope): void;
}

export function SheetDesignInspector({
  actions,
  visuals,
  saveLayout,
  mediaItems = [],
  mediaPreviewUrls,
  scope,
  sheet,
  onScopeChange,
}: SheetDesignInspectorProps) {
  const [hoveredScope, setHoveredScope] = useState<SheetDesignScope | null>(
    null,
  );
  const [pending, setPending] = useState(false);
  const pendingRef = useRef(false);
  const disabled = !actions || actions.disabled || pending;
  const backgroundValues = visualValues(sheet, scope, "background", visuals);
  const overlayValues = visualValues(sheet, scope, "overlay", visuals);
  const apply = async (operation: (actions: SheetDesignActions, scope: DecorativeScope) => Promise<boolean>) => {
    if (!actions || disabled || pendingRef.current) return;
    pendingRef.current = true;
    setPending(true);
    try {
      await operation(actions, scope === "both" ? "bothSides" : scope);
    } finally {
      pendingRef.current = false;
      setPending(false);
    }
  };

  return (
    <div className="sheet-design-inspector">
      <SheetScopePreview
        hoveredScope={hoveredScope}
        mediaPreviewUrls={mediaPreviewUrls}
        scope={scope}
        sheet={sheet}
        onHoveredScopeChange={setHoveredScope}
        onScopeChange={onScopeChange}
      />
      <p aria-live="polite" className="ui-section-eyebrow sheet-design-scope-status">
        {scopeLabel(scope)}
      </p>

      <SheetVisualControls
        key={`${sheet.sheetId}:${scope}`}
        disabled={disabled}
        backgroundValues={backgroundValues}
        overlayValues={overlayValues}
        mediaItems={mediaItems}
        mediaPreviewUrls={mediaPreviewUrls}
        onChange={(change) => { void apply((actions, target) => actions.onChange(sheet.sheetId, target, change)); }}
        onSelectMedia={(role, mediaId) => { void apply((actions, target) => actions.onApplyDecorative(sheet.sheetId, target, role, mediaId)); }}
      />

      <div className="sheet-design-save-layout">
        <ActionButton
          density="compact"
          disabled={!saveLayout?.enabled}
          onClick={saveLayout?.onSave}
          title={
            sheet.frames.length === 0
              ? "Adicione ao menos um quadro para salvar um layout."
              : "Salvar a disposição dos quadros em Personalizados."
          }
          type="button"
          variant="integrated"
        >
          Salvar disposição como layout
        </ActionButton>
        {saveLayout?.feedback}
      </div>
    </div>
  );
}

function SheetScopePreview({
  hoveredScope,
  mediaPreviewUrls,
  scope,
  sheet,
  onHoveredScopeChange,
  onScopeChange,
}: {
  hoveredScope: SheetDesignScope | null;
  mediaPreviewUrls: Readonly<Record<string, string>>;
  scope: SheetDesignScope;
  sheet: ComposedSheet;
  onHoveredScopeChange(scope: SheetDesignScope | null): void;
  onScopeChange(scope: SheetDesignScope): void;
}) {
  const inactiveSide =
    sheet.activeSides === "both"
      ? null
      : sheet.activeSides === "left"
        ? "right"
        : "left";
  const visualWidthUm =
    sheet.activeSides === "both" ? sheet.widthUm : sheet.widthUm * 2;

  return (
    <div
      aria-label={`Aplicar na lâmina ${String(sheet.number).padStart(2, "0")}`}
      className="sheet-design-preview"
      data-active-sides={sheet.activeSides}
      data-hovered-scope={hoveredScope ?? undefined}
      data-selected-scope={scope}
      role="group"
      style={
        {
          "--sheet-design-aspect-ratio": `${visualWidthUm} / ${sheet.heightUm}`,
        } as CSSProperties
      }
      onPointerLeave={() => onHoveredScopeChange(null)}
    >
      <SheetPreview
        mediaPreviewUrls={mediaPreviewUrls}
        sheet={sheet}
      />
      {inactiveSide ? (
        <span
          aria-hidden="true"
          className="sheet-design-preview__inactive"
          data-side={inactiveSide}
        />
      ) : null}
      <span
        aria-hidden="true"
        className="sheet-design-preview__highlight sheet-design-preview__highlight--selected"
        data-scope={scope}
      />
      {hoveredScope && hoveredScope !== scope ? (
        <span
          aria-hidden="true"
          className="sheet-design-preview__highlight sheet-design-preview__highlight--hovered"
          data-scope={hoveredScope}
        />
      ) : null}
      <VisualScopeControls
        activeSides={sheet.activeSides}
        targetLayout="overlaidCenter"
        labels={{ left: "Página esquerda", both: "Ambos os lados", right: "Página direita" }}
        scope={scope}
        focusPresentation="target"
        onScopeChange={onScopeChange}
        onHoveredScopeChange={onHoveredScopeChange}
        onFocusedScopeChange={onHoveredScopeChange}
      />
    </div>
  );
}

type VisualValue = (
  | { kind: "color"; label: string; rgb: string }
  | { kind: "media"; label: string; mediaId: string }
  | { kind: "none"; label: string }
) & { custom: boolean; side?: string };

function SheetVisualControls({
  disabled, backgroundValues, overlayValues, mediaItems, mediaPreviewUrls, onChange, onSelectMedia,
}: {
  disabled: boolean;
  backgroundValues: readonly VisualValue[];
  overlayValues: readonly VisualValue[];
  mediaItems: readonly MediaCatalogItem[];
  mediaPreviewUrls: Readonly<Record<string, string>>;
  onChange(change: SheetVisualChange): void;
  onSelectMedia(role: DecorativeRole, mediaId: string): void;
}) {
  const [openPicker, setOpenPicker] = useState<"Fundo" | "Sobreposição" | null>(null);
  const decorativeMedia = mediaItems.filter((media) => media.kind === "decorative");
  return <div className="sheet-design-controls">
    {(["Fundo", "Sobreposição"] as const).map((role) => {
      const background = role === "Fundo";
      const decorativeRole = background ? "background" : "overlay";
      const values = background ? backgroundValues : overlayValues;
      const description = values.map(value =>
        `${value.side ? value.side + ": " : ""}${value.label}. ${value.custom ? "Personalizado nesta lâmina" : "Usando o padrão do álbum"}.`,
      ).join(" ");
      return <section className="sheet-design-role" aria-label={role} key={role}>
        <VisualDesignControl
          decorativeMedia={decorativeMedia}
          disabled={disabled}
          label={role}
          mediaPreviewUrls={mediaPreviewUrls}
          description={description}
          open={openPicker === role}
          values={values}
          color={background ? { label: "do fundo da lâmina", onCommit: (rgb) => onChange({ kind: "backgroundColor", rgb }) } : undefined}
          onClear={background ? undefined : () => onChange({ kind: "remove", role: "overlay" })}
          onOpenChange={(open) => setOpenPicker(open ? role : null)}
          onSelect={(mediaId) => onSelectMedia(decorativeRole, mediaId)}
          actions={<>
            {background && <ActionButton aria-label="Remover" title="Remover fundo" density="compact" disabled={disabled}
              type="button" variant="integrated" onClick={() => onChange({ kind: "remove", role: decorativeRole })}>
              <AppIcon icon={X} size={14} />
            </ActionButton>}
            {values.some(value => value.custom) && <ActionButton aria-label="Usar padrão do álbum" title="Usar padrão do álbum"
              className="sheet-design-role__restore" density="compact" disabled={disabled}
              type="button" variant="integrated" onClick={() => onChange({ kind: "restoreAlbum", role: decorativeRole })}>
              <AppIcon icon={RotateCcw} size={14} />
            </ActionButton>}
          </>}
        />
      </section>;
    })}
  </div>;
}

function visualValues(
  sheet: ComposedSheet,
  scope: SheetDesignScope,
  role: "background" | "overlay",
  visuals?: SheetVisuals,
): VisualValue[] {
  if (scope !== "both") {
    return [visualValueAtSide(sheet, scope, role, visuals)];
  }
  const left = visualValueAtSide(sheet, "left", role, visuals);
  const right = visualValueAtSide(sheet, "right", role, visuals);
  if (sameVisualValue(left, right)) return [left];
  return [
    { ...left, side: "Esquerda" },
    { ...right, side: "Direita" },
  ];
}

function visualValueAtSide(
  sheet: ComposedSheet,
  side: "left" | "right",
  role: "background" | "overlay",
  visuals?: SheetVisuals,
): VisualValue {
  const visual = visuals?.[role];
  const custom = visual?.kind === "bothSides" || (visual?.kind === "perSide" && visual[side].kind === "custom");
  const sampleX = sheet.widthUm * (side === "left" ? 0.25 : 0.75);
  if (role === "overlay") {
    const overlay = [...sheet.overlays]
      .reverse()
      .find(({ drawRect, clipRect }) => containsX(clipRect ?? drawRect, sampleX));
    return overlay
      ? { custom, kind: "media", label: overlay.name, mediaId: overlay.mediaId }
      : { custom, kind: "none", label: "Sem sobreposição" };
  }

  const background = [...sheet.backgrounds]
    .reverse()
    .find((background) => containsX(background.kind === "media" ? background.clipRect ?? background.drawRect : background.drawRect, sampleX));
  if (!background) {
    return { custom, kind: "color", label: sheet.base.rgb, rgb: sheet.base.rgb };
  }
  return background.kind === "color"
    ? { custom, kind: "color", label: background.rgb, rgb: background.rgb }
    : {
        custom,
        kind: "media",
        label: background.name,
        mediaId: background.mediaId,
      };
}

function containsX(rect: { x: number; width: number }, sampleX: number) {
  return sampleX >= rect.x && sampleX <= rect.x + rect.width;
}

function sameVisualValue(left: VisualValue, right: VisualValue) {
  if (left.kind !== right.kind || left.custom !== right.custom) return false;
  if (left.kind === "color" && right.kind === "color") {
    return left.rgb === right.rgb;
  }
  if (left.kind === "media" && right.kind === "media") {
    return left.mediaId === right.mediaId;
  }
  return left.kind === "none" && right.kind === "none";
}

function scopeLabel(scope: SheetDesignScope) {
  if (scope === "left") return "Página esquerda";
  if (scope === "right") return "Página direita";
  return "Ambos os lados";
}
