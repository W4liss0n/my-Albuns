import { useState, type CSSProperties } from "react";

import type { VisualScope } from "../application/scopedValues";
import type { ComposedSheet } from "../domain/project";
import { PersonalizationPreview } from "../ui/visualPreview/PersonalizationPreview";
import type { VisualPersonalizationPreview, VisualPreviewGeometry } from "../ui/visualPreview";
import { SheetPreview } from "./SheetPreview";
import "./VisualScopePreview.css";

type PreviewContent =
  | {
      kind: "general";
      label: string;
      geometry: VisualPreviewGeometry;
      personalization: Omit<VisualPersonalizationPreview, "fixedScope">;
      frameGapUm: number;
      technicalGuides?: boolean;
    }
  | {
      kind: "sheet";
      sheet: ComposedSheet;
      mediaPreviewUrls: Readonly<Record<string, string>>;
    };

interface VisualScopePreviewProps {
  content: PreviewContent;
  label: string;
  scope: VisualScope;
  onScopeChange(scope: VisualScope): void;
  bothSidesControl?: "center" | "outside";
  focus?: { value: VisualScope | null; onChange(scope: VisualScope | null): void };
}

const SIDES = ["left", "right"] as const;
const LABELS = { left: "Página esquerda", both: "Ambos os lados", right: "Página direita" };
const SCOPE_SUFFIX = { left: "do lado esquerdo", both: "de ambos os lados", right: "do lado direito" };

// One interaction surface for draft examples and the current composed sheet.
// Content renderers do not own selection, hover, keyboard focus or hit areas.
export function VisualScopePreview({
  content, label, scope, onScopeChange, bothSidesControl = "center", focus,
}: VisualScopePreviewProps) {
  const [hoveredScope, setHoveredScope] = useState<VisualScope | null>(null);
  const [localFocus, setLocalFocus] = useState<VisualScope | null>(null);
  const activeSides = content.kind === "sheet" ? content.sheet.activeSides : "both";
  const available = (value: VisualScope | null) =>
    activeSides === "both" || value === activeSides ? value : null;
  const hovered = available(hoveredScope);
  const focused = available(focus ? focus.value : localFocus);
  const setFocused = focus ? focus.onChange : setLocalFocus;
  const selected = activeSides === "both" ? scope : activeSides;
  const hasCenter = activeSides === "both" && bothSidesControl === "center";
  const targets: readonly VisualScope[] = activeSides !== "both"
    ? [activeSides] : hasCenter ? ["left", "both", "right"] : SIDES;
  const height = content.kind === "sheet" ? content.sheet.heightUm : content.geometry.heightUm;
  const width = content.kind === "sheet"
    ? content.sheet.widthUm * (activeSides === "both" ? 1 : 2)
    : content.geometry.widthUm;
  const includes = (target: VisualScope | null, side: VisualScope) => target === "both" || target === side;
  const focusInset = height * 0.012;
  const focusStroke = Math.max(1, height * 0.0035);

  return (
    <div
      aria-label={label}
      className="visual-scope-preview"
      data-active-sides={activeSides}
      data-hovered-scope={hovered ?? undefined}
      data-focused-scope={focused ?? undefined}
      data-selected-scope={selected}
      role="group"
      style={{ "--visual-scope-aspect-ratio": `${width} / ${height}` } as CSSProperties}
      onPointerLeave={() => setHoveredScope(null)}
    >
      <div className="visual-scope-preview__content">
        {content.kind === "general" ? (
          <PersonalizationPreview
            accessibleLabel={content.label}
            frameGapUm={content.frameGapUm}
            geometry={content.geometry}
            personalization={content.personalization}
            frameOpacities={SIDES.map(side => includes(selected, side) ? 0.24 : includes(focused, side) ? 0.15 : 0.08)}
            showTechnicalGuides={content.technicalGuides ?? false}
          />
        ) : (
          <SheetPreview sheet={content.sheet} mediaPreviewUrls={content.mediaPreviewUrls} />
        )}
      </div>
      {activeSides !== "both" && (
        <span aria-hidden="true" className="visual-scope-preview__inactive" data-side={activeSides === "left" ? "right" : "left"} />
      )}
      <svg className="visual-scope-preview__state" viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
        {SIDES.map((side, index) => activeSides === "both" && !includes(selected, side) ? (
          <rect
            key={side}
            aria-label={`Lado não selecionado: ${side === "left" ? "esquerdo" : "direito"}`}
            fill="#E3E0DA"
            fillOpacity={includes(hovered, side) || includes(focused, side) ? "0.18" : "0.42"}
            stroke="none"
            x={index * width / 2} y="0" width={width / 2} height={height}
          />
        ) : null)}
      </svg>
      <span aria-hidden="true" className="visual-scope-preview__selection" data-scope={selected} />
      <div className="visual-scope-controls" data-center={hasCenter || undefined}>
        {hovered && <span aria-hidden="true" aria-label={`Pré-seleção ${SCOPE_SUFFIX[hovered]}`} className="visual-scope-controls__hover" data-scope={hovered} />}
        {targets.map(target => (
          <button
            aria-label={LABELS[target]}
            aria-pressed={selected === target}
            className="visual-scope-controls__target"
            data-scope={target}
            key={target}
            type="button"
            onClick={() => onScopeChange(target)}
            onFocus={event => setFocused(event.currentTarget.matches(":focus-visible") ? target : null)}
            onBlur={() => setFocused(null)}
            onPointerEnter={() => setHoveredScope(target)}
            onPointerLeave={() => setHoveredScope(null)}
          />
        ))}
      </div>
      {focused && (
        <svg className="visual-scope-preview__focus" viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
          <rect
            aria-label={`Foco de teclado ${SCOPE_SUFFIX[focused]}`}
            fill="none" stroke="#73A9CE" strokeWidth={focusStroke}
            strokeDasharray={`${focusStroke * 0.1} ${height * 0.018}`} strokeLinecap="round"
            x={(focused === "right" ? width / 2 : 0) + focusInset}
            y={focusInset}
            width={Math.max(1, (focused === "both" ? width : width / 2) - focusInset * 2)}
            height={Math.max(1, height - focusInset * 2)}
          />
        </svg>
      )}
    </div>
  );
}
