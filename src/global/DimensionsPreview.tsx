import {
  displayUnitLabel,
  formatMicrometers,
  type NewProjectDimensionsDraft,
} from "./application/newProjectDimensions";

interface DimensionsPreviewProps {
  draft: NewProjectDimensionsDraft;
}

export function DimensionsPreview({ draft }: DimensionsPreviewProps) {
  const width = Math.max(1, draft.closedSheetWidth.valueUm * 2);
  const height = Math.max(1, draft.sheetHeight.valueUm);
  const smallestSide = Math.min(width, height);
  const bleed = Math.max(
    0,
    Math.min(draft.bleed.valueUm, smallestSide / 4),
  );
  const safety = Math.max(
    0,
    Math.min(draft.safety.valueUm, smallestSide / 4),
  );
  const safetyInset = bleed + safety;

  return (
    <section className="new-project-dimensions-preview">
      <p className="new-project-preview-metadata">
        <span>Lâmina aberta</span>
        <strong>
          {formatMicrometers(width, draft.displayUnit)} × {draft.sheetHeight.text}{" "}
          {displayUnitLabel(draft.displayUnit)}
        </strong>
        <span>· {draft.sheetCountText || "0"} Lâminas</span>
        <span className="new-project-guide new-project-guide--bleed">
          sangria
        </span>
        <span className="new-project-guide new-project-guide--safety">
          Área de segurança
        </span>
      </p>
      <div className="new-project-dimensions-stage">
        <svg
          aria-label="Prévia das Dimensões"
          height={height}
          preserveAspectRatio="xMidYMid meet"
          role="img"
          viewBox={`0 0 ${width} ${height}`}
          width={width}
          xmlns="http://www.w3.org/2000/svg"
        >
          <title>Prévia das Dimensões</title>
          <rect fill="#fbfaf8" height={height} width={width} />
          <line
            stroke="#d7d2c8"
            strokeWidth={Math.max(1, height * 0.003)}
            x1={width / 2}
            x2={width / 2}
            y1={0}
            y2={height}
          />
          <rect
            fill="none"
            height={Math.max(1, height - bleed * 2)}
            stroke="#c57c70"
            strokeDasharray={`${Math.max(2, height * 0.01)} ${Math.max(2, height * 0.008)}`}
            strokeWidth={Math.max(1, height * 0.003)}
            width={Math.max(1, width - bleed * 2)}
            x={bleed}
            y={bleed}
          />
          <rect
            fill="none"
            height={Math.max(1, height - safetyInset * 2)}
            stroke="#6f9fbe"
            strokeDasharray={`${Math.max(2, height * 0.01)} ${Math.max(2, height * 0.008)}`}
            strokeWidth={Math.max(1, height * 0.003)}
            width={Math.max(1, width - safetyInset * 2)}
            x={safetyInset}
            y={safetyInset}
          />
        </svg>
      </div>
      <p className="new-project-preview-caption">
        Proporção real da Lâmina. A personalização visual é definida no passo 2.
      </p>
    </section>
  );
}
