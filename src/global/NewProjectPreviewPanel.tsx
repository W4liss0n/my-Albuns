import type { ReactNode } from "react";

import {
  displayUnitLabel,
  formatMicrometers,
  type NewProjectDimensionsDraft,
} from "./application/newProjectDimensions";
import {
  ProportionalPreviewViewport,
  type PreviewOutsideSurfaceAction,
} from "./ProportionalPreviewViewport";
import {
  createNewProjectPreviewGeometry,
  type NewProjectPreviewGeometry,
} from "./newProjectPreviewGeometry";

import "./NewProjectPreviewPanel.css";

interface NewProjectPreviewPanelProps {
  children(geometry: NewProjectPreviewGeometry): ReactNode;
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
      <p className="new-project-preview-metadata">
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
      <ProportionalPreviewViewport
        height={geometry.heightUm}
        label={surfaceLabel}
        outsideSurfaceAction={outsideSurfaceAction}
        width={geometry.widthUm}
      >
        {children(geometry)}
      </ProportionalPreviewViewport>
      <p className="new-project-preview-caption">
        Proporção real da Lâmina aberta.
      </p>
    </section>
  );
}
