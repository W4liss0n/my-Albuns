import { useState } from "react";

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
  focus:
    | { kind: "local" }
    | {
        kind: "controlled";
        value: VisualScope | null;
        onChange(scope: VisualScope | null): void;
      };
  frameGapUm: number;
  geometry: VisualPreviewGeometry;
  personalization: VisualPersonalizationPreview;
  presentation: PersonalizationScopeSurfacePresentation;
  onScopeChange(scope: VisualScope): void;
}

export function PersonalizationScopeSurface({
  includeBothSidesControl = false,
  focus,
  frameGapUm,
  geometry,
  personalization,
  presentation,
  onScopeChange,
}: PersonalizationScopeSurfaceProps) {
  const [localFocusedScope, setLocalFocusedScope] =
    useState<VisualScope | null>(null);
  const [hoveredScope, setHoveredScope] = useState<VisualScope | null>(null);
  const focusedScope =
    focus.kind === "controlled" ? focus.value : localFocusedScope;
  const setFocusedScope =
    focus.kind === "controlled" ? focus.onChange : setLocalFocusedScope;
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
            onBlur={() => setFocusedScope(null)}
            onClick={() => onScopeChange(scope)}
            onFocus={(event) =>
              setFocusedScope(
                event.currentTarget.matches(":focus-visible") ? scope : null,
              )
            }
            onPointerEnter={() => setHoveredScope(scope)}
            onPointerLeave={() =>
              setHoveredScope(
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
