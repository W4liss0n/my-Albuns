import type { CSSProperties, ReactNode } from "react";

import {
  displayUnitLabel,
  formatMicrometers,
} from "../application/physicalMeasurements";
import {
  ProportionalPreviewViewport,
  type VisualPreviewGeometry,
} from "../ui/visualPreview";
import { SHEET_GUIDE_STYLE } from "../ui/sheetGuideGeometry";
import type { NewProjectDimensionsDraft } from "./application/newProjectDimensions";
import { createNewProjectPreviewGeometry } from "./newProjectPreviewGeometry";

import "./NewProjectPreviewPanel.css";

interface PreviewOutsideSurfaceAction {
  label: string;
  onFocusChange(focused: boolean): void;
  onPress(): void;
  pressed: boolean;
}

interface NewProjectPreviewPanelProps {
  children(geometry: VisualPreviewGeometry): ReactNode;
  draft: NewProjectDimensionsDraft;
  outsideSurfaceAction?: PreviewOutsideSurfaceAction;
  surfaceLabel?: string;
}

export function NewProjectPreviewPanel({
  children,
  draft,
  outsideSurfaceAction,
  surfaceLabel,
}: NewProjectPreviewPanelProps) {
  const geometry = createNewProjectPreviewGeometry(draft);

  return (
    <section
      aria-label="Prévia da Lâmina aberta"
      className="new-project-preview-panel"
      onClick={() => outsideSurfaceAction?.onPress()}
    >
      <p
        className="new-project-preview-metadata"
        style={
          {
            "--new-project-guide-bleed": SHEET_GUIDE_STYLE.bleed,
            "--new-project-guide-safety": SHEET_GUIDE_STYLE.safety,
          } as CSSProperties
        }
      >
        <span>Lâmina aberta</span>
        <strong>
          {formatMicrometers(geometry.widthUm, draft.displayUnit)} ×{" "}
          {draft.sheetHeight.text} {displayUnitLabel(draft.displayUnit)}
        </strong>
        <span>· {draft.sheetCountText || "0"} Lâminas</span>
        <span className="new-project-guide new-project-guide--bleed">
          sangria
        </span>
        <span className="new-project-guide new-project-guide--safety">
          Área de segurança
        </span>
      </p>
      <div className="new-project-preview-viewport">
        {outsideSurfaceAction ? (
          <button
            aria-label={outsideSurfaceAction.label}
            aria-pressed={outsideSurfaceAction.pressed}
            className="new-project-preview-outside-action"
            onBlur={() => outsideSurfaceAction.onFocusChange(false)}
            onClick={(event) => {
              event.stopPropagation();
              outsideSurfaceAction.onPress();
            }}
            onFocus={(event) =>
              outsideSurfaceAction.onFocusChange(
                event.currentTarget.matches(":focus-visible"),
              )
            }
            type="button"
          />
        ) : null}
        <ProportionalPreviewViewport
          height={geometry.heightUm}
          label={surfaceLabel}
          width={geometry.widthUm}
        >
          {children(geometry)}
        </ProportionalPreviewViewport>
      </div>
      <p className="new-project-preview-caption">
        Proporção real da Lâmina aberta.
      </p>
    </section>
  );
}
