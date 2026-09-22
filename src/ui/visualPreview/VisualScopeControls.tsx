import type { VisualScope } from "../../application/scopedValues";
import "./VisualScopeControls.css";

interface VisualScopeControlsProps {
  activeSides?: VisualScope;
  targetLayout: "halves" | "partitionedCenter" | "overlaidCenter";
  groupLabel?: string;
  labels: Readonly<Record<VisualScope, string>>;
  scope: VisualScope;
  hoveredScope: VisualScope | null;
  focusPresentation: "preview" | "target";
  onScopeChange(scope: VisualScope): void;
  onHoveredScopeChange(scope: VisualScope | null): void;
  onFocusedScopeChange(scope: VisualScope | null): void;
}

// Owns target geometry, available sides and pointer/keyboard interpretation.
// Owns the full-page hover tint; consumers keep selection and keyboard focus.
export function VisualScopeControls({
  activeSides = "both",
  targetLayout,
  groupLabel,
  labels,
  scope,
  hoveredScope,
  focusPresentation,
  onScopeChange,
  onHoveredScopeChange,
  onFocusedScopeChange,
}: VisualScopeControlsProps) {
  const hasCenter = activeSides === "both" && targetLayout !== "halves";
  const scopes: readonly VisualScope[] = activeSides !== "both"
    ? [activeSides]
    : hasCenter ? ["left", "both", "right"] : ["left", "right"];

  return (
    <div
      aria-label={groupLabel}
      className="visual-scope-controls"
      data-focus-presentation={focusPresentation}
      data-target-layout={hasCenter ? targetLayout : "halves"}
      role={groupLabel ? "group" : undefined}
      onPointerLeave={() => onHoveredScopeChange(null)}
    >
      {hoveredScope ? (
        <span
          aria-hidden="true"
          aria-label={`Pré-seleção ${hoveredScope === "both" ? "de ambos os lados" : hoveredScope === "left" ? "do lado esquerdo" : "do lado direito"}`}
          className="visual-scope-controls__hover"
          data-scope={hoveredScope}
        />
      ) : null}
      {scopes.map((candidate) => (
        <button
          aria-label={labels[candidate]}
          aria-pressed={scope === candidate}
          className="visual-scope-controls__target"
          data-scope={candidate}
          key={candidate}
          type="button"
          onBlur={() => onFocusedScopeChange(null)}
          onClick={() => onScopeChange(candidate)}
          onFocus={(event) => onFocusedScopeChange(
            focusPresentation === "target" || event.currentTarget.matches(":focus-visible")
              ? candidate : null,
          )}
          onPointerEnter={() => onHoveredScopeChange(candidate)}
          onPointerLeave={() => onHoveredScopeChange(null)}
        />
      ))}
    </div>
  );
}
