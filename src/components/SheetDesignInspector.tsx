import { useState, type CSSProperties } from "react";

import type { VisualScope } from "../application/scopedValues";
import type {
  ComposedSheet,
  ProjectedFrameBorder,
} from "../domain/project";
import { ActionButton } from "../ui";
import { SheetPreview } from "./SheetPreview";
import "./SheetDesignInspector.css";

export type SheetDesignScope = VisualScope;

interface SheetDesignInspectorProps {
  frameBorder: ProjectedFrameBorder;
  mediaPreviewUrls: Readonly<Record<string, string>>;
  scope: SheetDesignScope;
  sheet: ComposedSheet;
  onScopeChange(scope: SheetDesignScope): void;
}

export function SheetDesignInspector({
  frameBorder,
  mediaPreviewUrls,
  scope,
  sheet,
  onScopeChange,
}: SheetDesignInspectorProps) {
  const [hoveredScope, setHoveredScope] = useState<SheetDesignScope | null>(
    null,
  );
  const backgroundValues = visualValues(sheet, scope, "background");
  const overlayValues = visualValues(sheet, scope, "overlay");

  return (
    <div className="sheet-design-inspector">
      <SheetScopePreview
        frameBorder={frameBorder}
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
        placeholderFeature="edit-sheet-background"
        role="Background"
        values={backgroundValues}
        mediaPreviewUrls={mediaPreviewUrls}
      />
      <SheetVisualRole
        placeholderFeature="edit-sheet-overlay"
        role="Overlay"
        values={overlayValues}
        mediaPreviewUrls={mediaPreviewUrls}
      />

      <ActionButton
        data-placeholder-feature="save-sheet-layout"
        density="compact"
        disabled
        title={
          sheet.frames.length === 0
            ? "Adicione ao menos um Frame para salvar um Layout."
            : "Salvar disposição como Layout ainda não está disponível nesta versão."
        }
        type="button"
        variant="secondary"
      >
        Salvar disposição como Layout
      </ActionButton>
    </div>
  );
}

function SheetScopePreview({
  frameBorder,
  hoveredScope,
  mediaPreviewUrls,
  scope,
  sheet,
  onHoveredScopeChange,
  onScopeChange,
}: {
  frameBorder: ProjectedFrameBorder;
  hoveredScope: SheetDesignScope | null;
  mediaPreviewUrls: Readonly<Record<string, string>>;
  scope: SheetDesignScope;
  sheet: ComposedSheet;
  onHoveredScopeChange(scope: SheetDesignScope | null): void;
  onScopeChange(scope: SheetDesignScope): void;
}) {
  const scopes = availableScopes(sheet);
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
      aria-label={`Selecionar escopo da Lâmina ${String(sheet.number).padStart(2, "0")}`}
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
      onMouseLeave={() => onHoveredScopeChange(null)}
    >
      <SheetPreview
        frameBorder={frameBorder}
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
      <div className="sheet-design-preview__targets">
        {scopes.map((candidate) => (
          <button
            aria-label={scopeLabel(candidate)}
            aria-pressed={scope === candidate}
            className="sheet-design-preview__target"
            data-scope={candidate}
            key={candidate}
            type="button"
            onBlur={() => onHoveredScopeChange(null)}
            onClick={() => onScopeChange(candidate)}
            onFocus={() => onHoveredScopeChange(candidate)}
            onMouseEnter={() => onHoveredScopeChange(candidate)}
          />
        ))}
      </div>
    </div>
  );
}

type VisualValue =
  | { kind: "color"; label: string; rgb: string; side?: string }
  | { kind: "media"; label: string; mediaId: string; side?: string }
  | { kind: "none"; label: string; side?: string };

function SheetVisualRole({
  mediaPreviewUrls,
  placeholderFeature,
  role,
  values,
}: {
  mediaPreviewUrls: Readonly<Record<string, string>>;
  placeholderFeature: string;
  role: "Background" | "Overlay";
  values: readonly VisualValue[];
}) {
  return (
    <section className="sheet-design-role">
      <h3>{role}</h3>
      <div className="sheet-design-role__values">
        {values.map((value, index) => (
          <div
            className="sheet-design-value"
            key={`${value.side ?? "both"}-${index}`}
          >
            <VisualSwatch mediaPreviewUrls={mediaPreviewUrls} value={value} />
            <span className="sheet-design-value__copy">
              {value.side ? <small>{value.side}</small> : null}
              <strong>{value.label}</strong>
              <small data-placeholder-feature="sheet-design-origin">
                Origem ainda não disponível
              </small>
            </span>
          </div>
        ))}
      </div>
      <div className="sheet-design-role__actions">
        {role === "Background" ? (
          <label className="sheet-design-color-placeholder">
            <span>Cor</span>
            <input
              aria-label="Cor do Background da Lâmina"
              data-placeholder-feature={placeholderFeature}
              disabled
              type="color"
              value={firstBackgroundColor(values)}
              readOnly
            />
          </label>
        ) : null}
        <ActionButton
          data-placeholder-feature={placeholderFeature}
          density="compact"
          disabled
          title={`${role} da Lâmina ainda não pode ser alterado nesta versão.`}
          type="button"
          variant="quiet"
        >
          Remover
        </ActionButton>
      </div>
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
            : undefined
      }
    />
  );
}

function availableScopes(sheet: ComposedSheet): SheetDesignScope[] {
  if (sheet.activeSides === "both") return ["left", "both", "right"];
  return [sheet.activeSides];
}

function visualValues(
  sheet: ComposedSheet,
  scope: SheetDesignScope,
  role: "background" | "overlay",
): VisualValue[] {
  if (scope !== "both") {
    return [visualValueAtSide(sheet, scope, role)];
  }
  const left = visualValueAtSide(sheet, "left", role);
  const right = visualValueAtSide(sheet, "right", role);
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
): VisualValue {
  const sampleX = sheet.widthUm * (side === "left" ? 0.25 : 0.75);
  if (role === "overlay") {
    const overlay = [...sheet.overlays]
      .reverse()
      .find(({ drawRect }) => containsX(drawRect, sampleX));
    return overlay
      ? { kind: "media", label: overlay.name, mediaId: overlay.mediaId }
      : { kind: "none", label: "Sem overlay" };
  }

  const background = [...sheet.backgrounds]
    .reverse()
    .find(({ drawRect }) => containsX(drawRect, sampleX));
  if (!background) {
    return { kind: "color", label: sheet.base.rgb, rgb: sheet.base.rgb };
  }
  return background.kind === "color"
    ? { kind: "color", label: background.rgb, rgb: background.rgb }
    : {
        kind: "media",
        label: background.name,
        mediaId: background.mediaId,
      };
}

function containsX(rect: { x: number; width: number }, sampleX: number) {
  return sampleX >= rect.x && sampleX <= rect.x + rect.width;
}

function sameVisualValue(left: VisualValue, right: VisualValue) {
  if (left.kind !== right.kind) return false;
  if (left.kind === "color" && right.kind === "color") {
    return left.rgb === right.rgb;
  }
  if (left.kind === "media" && right.kind === "media") {
    return left.mediaId === right.mediaId;
  }
  return left.kind === "none" && right.kind === "none";
}

function firstBackgroundColor(values: readonly VisualValue[]) {
  return values.find((value) => value.kind === "color")?.rgb ?? "#FFFFFF";
}

function scopeLabel(scope: SheetDesignScope) {
  if (scope === "left") return "Página esquerda";
  if (scope === "right") return "Página direita";
  return "Ambos os lados";
}
