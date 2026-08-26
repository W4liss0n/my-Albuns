import type {
  NewProjectDimensionsDraft,
} from "./application/newProjectDimensions";
import { SheetGuideLayer } from "../ui/visualPreview";
import "../ui/visualPreview/VisualPreviewSheet.css";
import { NewProjectPreviewPanel } from "./NewProjectPreviewPanel";

interface DimensionsPreviewProps {
  draft: NewProjectDimensionsDraft;
}

export function DimensionsPreview({ draft }: DimensionsPreviewProps) {
  return (
    <NewProjectPreviewPanel
      draft={draft}
      surfaceLabel="Prévia do formato da Lâmina"
    >
      {(geometry) => {
        const { heightUm, widthUm } = geometry;
        return (
          <svg
            aria-label="Prévia das Dimensões"
            className="visual-preview-sheet"
            height={heightUm}
            preserveAspectRatio="xMidYMid meet"
            role="img"
            viewBox={`0 0 ${widthUm} ${heightUm}`}
            width={widthUm}
            xmlns="http://www.w3.org/2000/svg"
          >
            <title>Prévia das Dimensões</title>
            <rect fill="#fbfaf8" height={heightUm} width={widthUm} />
            <SheetGuideLayer geometry={geometry} />
          </svg>
        );
      }}
    </NewProjectPreviewPanel>
  );
}
