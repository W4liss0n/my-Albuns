import type { VisualScope } from "../../application/scopedValues";
import type {
  VisualPersonalizationPreview,
  VisualPreviewGeometry,
} from "./types";
import { PersonalizationPreview } from "./PersonalizationPreview";

export interface PersonalizationScopeSurfacePresentation {
  accessiblePreviewLabel: string;
  externalSelection: boolean;
  scopeControlsLabel: string;
  technicalGuides: boolean;
}

interface PersonalizationScopeSurfaceProps {
  includeBothSidesControl?: boolean;
  focusedScope: VisualScope | null;
  frameGapUm: number;
  geometry: VisualPreviewGeometry;
  hoveredScope: VisualScope | null;
  personalization: VisualPersonalizationPreview;
  presentation: PersonalizationScopeSurfacePresentation;
  onFocusedScopeChange(scope: VisualScope | null): void;
  onHoveredScopeChange(scope: VisualScope | null): void;
  onScopeChange(scope: VisualScope): void;
}

export function PersonalizationScopeSurface({
  includeBothSidesControl = false,
  focusedScope,
  frameGapUm,
  geometry,
  hoveredScope,
  personalization,
  presentation,
  onFocusedScopeChange,
  onHoveredScopeChange,
  onScopeChange,
}: PersonalizationScopeSurfaceProps) {
  const previewedScope =
    personalization.fixedScope === "both" ||
    personalization.fixedScope === hoveredScope
      ? null
      : hoveredScope;

  return (
    <>
      <PersonalizationPreview
        accessibleLabel={presentation.accessiblePreviewLabel}
        frameGapUm={frameGapUm}
        geometry={geometry}
        hoveredScope={previewedScope}
        personalization={personalization}
        focusedScope={focusedScope}
        showTechnicalGuides={presentation.technicalGuides}
      />
      {presentation.externalSelection ? (
        <div
          aria-hidden="true"
          className={`visual-preview-fixed-selection visual-preview-fixed-selection--${personalization.fixedScope}`}
        />
      ) : null}
      <div
        aria-label={presentation.scopeControlsLabel}
        className={`visual-preview-scope-controls${
          includeBothSidesControl
            ? " visual-preview-scope-controls--with-both"
            : ""
        }`}
        role="group"
      >
        {(
          includeBothSidesControl
            ? ([
                ["left", "Lado esquerdo"],
                ["both", "Ambos os lados"],
                ["right", "Lado direito"],
              ] as const)
            : ([
                ["left", "Lado esquerdo"],
                ["right", "Lado direito"],
              ] as const)
        ).map(([scope, label]) => (
          <button
            aria-label={label}
            aria-pressed={personalization.fixedScope === scope}
            key={scope}
            onBlur={() => onFocusedScopeChange(null)}
            onClick={() => onScopeChange(scope)}
            onFocus={(event) =>
              onFocusedScopeChange(
                event.currentTarget.matches(":focus-visible") ? scope : null,
              )
            }
            onPointerEnter={() => onHoveredScopeChange(scope)}
            onPointerLeave={() =>
              onHoveredScopeChange(
                hoveredScope === scope ? null : hoveredScope,
              )
            }
            type="button"
          />
        ))}
      </div>
    </>
  );
}
