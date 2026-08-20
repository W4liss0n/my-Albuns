import type { NewProjectPersonalizationDraft } from "./application/newProjectPersonalization";
import type { NewProjectPreviewGeometry } from "./newProjectPreviewGeometry";
import { PersonalizationPreview } from "./PersonalizationPreview";

interface PersonalizationScopeSurfaceProps {
  includeBothSidesControl?: boolean;
  focusedScope: NewProjectPersonalizationDraft["fixedScope"] | null;
  frameGapUm: number;
  geometry: NewProjectPreviewGeometry;
  hoveredScope: NewProjectPersonalizationDraft["fixedScope"] | null;
  personalization: NewProjectPersonalizationDraft;
  onFocusedScopeChange(
    scope: NewProjectPersonalizationDraft["fixedScope"] | null,
  ): void;
  onHoveredScopeChange(
    scope: NewProjectPersonalizationDraft["fixedScope"] | null,
  ): void;
  onScopeChange(scope: NewProjectPersonalizationDraft["fixedScope"]): void;
}

export function PersonalizationScopeSurface({
  includeBothSidesControl = false,
  focusedScope,
  frameGapUm,
  geometry,
  hoveredScope,
  personalization,
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
        frameGapUm={frameGapUm}
        geometry={geometry}
        hoveredScope={previewedScope}
        personalization={personalization}
        focusedScope={focusedScope}
      />
      <div
        aria-hidden="true"
        className={`new-project-fixed-selection new-project-fixed-selection--${personalization.fixedScope}`}
      />
      <div
        aria-label="Escopo da personalização"
        className={`new-project-scope-controls${
          includeBothSidesControl
            ? " new-project-scope-controls--with-both"
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
