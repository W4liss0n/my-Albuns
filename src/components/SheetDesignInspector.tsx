import { useRef, useState, type CSSProperties, type ReactNode } from "react";

import type { VisualScope } from "../application/scopedValues";
import type {
  ComposedSheet, DecorativeScope, SheetVisualChange, SheetVisuals,
} from "../domain/project";
import { ActionButton } from "../ui";
import { ColorPropertyControl } from "../ui/ColorPropertyControl";
import { VisualScopeControls } from "../ui/visualPreview/VisualScopeControls";
import { SheetPreview } from "./SheetPreview";
import { SHEET_VISUAL_STYLE } from "./sheetVisualStyle";
import "./SheetDesignInspector.css";

export type SheetDesignScope = VisualScope;

export interface SheetDesignActions {
  disabled: boolean;
  onChange(sheetId: string, scope: DecorativeScope, change: SheetVisualChange): Promise<boolean>;
}

interface SheetDesignInspectorProps {
  actions?: SheetDesignActions;
  visuals?: SheetVisuals;
  saveLayout?: { enabled: boolean; onSave(): void; feedback?: ReactNode };
  mediaPreviewUrls: Readonly<Record<string, string>>;
  scope: SheetDesignScope;
  sheet: ComposedSheet;
  onScopeChange(scope: SheetDesignScope): void;
}

export function SheetDesignInspector({
  actions,
  visuals,
  saveLayout,
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
  const apply = async (change: SheetVisualChange) => {
    if (!actions || disabled || pendingRef.current) return;
    pendingRef.current = true;
    setPending(true);
    try {
      await actions.onChange(sheet.sheetId, scope === "both" ? "bothSides" : scope, change);
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
      <p aria-live="polite" className="sheet-design-scope-status">
        {scopeLabel(scope)}
      </p>

      <SheetVisualRole
        key={`background:${sheet.sheetId}:${scope}`}
        disabled={disabled}
        onChange={(change) => { void apply(change); }}
        role="Fundo"
        values={backgroundValues}
        mediaPreviewUrls={mediaPreviewUrls}
      />
      <SheetVisualRole
        disabled={disabled}
        onChange={(change) => { void apply(change); }}
        role="Sobreposição"
        values={overlayValues}
        mediaPreviewUrls={mediaPreviewUrls}
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
          variant="secondary"
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

function SheetVisualRole({ mediaPreviewUrls, disabled, role, values, onChange }: {
  mediaPreviewUrls: Readonly<Record<string, string>>;
  disabled: boolean;
  role: "Fundo" | "Sobreposição";
  values: readonly VisualValue[];
  onChange(change: SheetVisualChange): void;
}) {
  const decorativeRole = role === "Fundo" ? "background" : "overlay";
  return (
    <section className="sheet-design-role" aria-label={role}>
      <h3>{role}</h3>
      <div className="sheet-design-role__values">
        {values.map((value, index) => (
          <div className="sheet-design-value" key={value.side ?? index}>
            <VisualSwatch mediaPreviewUrls={mediaPreviewUrls} value={value} />
            <span className="sheet-design-value__copy">
              {value.side ? <small>{value.side}</small> : null}
              <strong>{value.label}</strong>
              <small>{value.custom ? "Personalizado nesta lâmina" : "Usando o padrão do álbum"}</small>
            </span>
          </div>
        ))}
      </div>
      <div className="sheet-design-role__actions">
        {role === "Fundo" ? (
          <div className="sheet-design-color">
            <span>Cor</span>
            <ColorPropertyControl label="do fundo da lâmina" defaultRgb="#FFFFFF"
              disabled={disabled} rgb={sharedBackgroundColor(values)}
              onCommit={(rgb) => onChange({ kind: "backgroundColor", rgb })} />
          </div>
        ) : null}
        <ActionButton density="compact" disabled={disabled} type="button" variant="quiet"
          onClick={() => onChange({ kind: "remove", role: decorativeRole })}>
          Remover
        </ActionButton>
      </div>
      {values.some((value) => value.custom) && (
        <ActionButton className="sheet-design-role__restore" density="compact" disabled={disabled}
          type="button" variant="quiet" onClick={() => onChange({ kind: "restoreAlbum", role: decorativeRole })}>
          Usar padrão do álbum
        </ActionButton>
      )}
    </section>
  );
}

function VisualSwatch({
  mediaPreviewUrls,
  value,
}: {
  mediaPreviewUrls: Readonly<Record<string, string>>;
  value: VisualValue;
}) {
  const previewUrl =
    value.kind === "media" ? mediaPreviewUrls[value.mediaId] : undefined;
  return (
    <span
      aria-hidden="true"
      className={`sheet-design-value__swatch sheet-design-value__swatch--${value.kind}`}
      style={
        value.kind === "color"
          ? { backgroundColor: value.rgb }
          : previewUrl
            ? { backgroundImage: `url("${previewUrl}")` }
            : value.kind === "media"
              ? { backgroundColor: SHEET_VISUAL_STYLE.mediaFallback.fill }
              : undefined
      }
    />
  );
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

function sharedBackgroundColor(values: readonly VisualValue[]) {
  const first = values[0];
  return first?.kind === "color" && values.every((value) => value.kind === "color" && value.rgb === first.rgb)
    ? first.rgb : null;
}

function scopeLabel(scope: SheetDesignScope) {
  if (scope === "left") return "Página esquerda";
  if (scope === "right") return "Página direita";
  return "Ambos os lados";
}
