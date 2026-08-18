import type { ReactNode } from "react";

import {
  displayUnitLabel,
  formatMicrometers,
  type NewProjectDimensionsDraft,
} from "./application/newProjectDimensions";
import { ProportionalPreviewViewport } from "./ProportionalPreviewViewport";

import "./NewProjectPreviewPanel.css";

interface NewProjectPreviewGeometry {
  bleedUm: number;
  heightUm: number;
  safetyUm: number;
  widthUm: number;
}

interface NewProjectPreviewPanelProps {
  children(geometry: NewProjectPreviewGeometry): ReactNode;
  draft: NewProjectDimensionsDraft;
  surfaceLabel?: string;
}

export function NewProjectPreviewPanel({
  children,
  draft,
  surfaceLabel,
}: NewProjectPreviewPanelProps) {
  const geometry = {
    bleedUm: draft.bleed.valueUm,
    heightUm: Math.max(1, draft.sheetHeight.valueUm),
    safetyUm: draft.safety.valueUm,
    widthUm: Math.max(1, draft.closedSheetWidth.valueUm * 2),
  } satisfies NewProjectPreviewGeometry;

  return (
    <section
      aria-label="Prévia da Lâmina aberta"
      className="new-project-preview-panel"
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
