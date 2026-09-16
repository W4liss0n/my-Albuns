import type { AlbumInformation, SheetSnapshot } from "../domain/project";
import type { ProjectedBackgroundContent } from "../domain/generated/ProjectedBackgroundContent";
import type { ProjectedOverlayContent } from "../domain/generated/ProjectedOverlayContent";
import type { SideVisual } from "../domain/generated/SideVisual";

export interface EdgeConversionLoss {
  readonly sheetId: string;
  readonly sheetNumber: number;
  readonly side: "left" | "right";
  readonly background: SideVisual<ProjectedBackgroundContent> | null;
  readonly overlay: SideVisual<ProjectedOverlayContent | null> | null;
}

/** Only per-page applications disappear; inherited and Both sides visuals survive. */
export function edgeConversionLoss(
  sheets: readonly SheetSnapshot[],
  sheetId: string,
): EdgeConversionLoss | null {
  const index = sheets.findIndex((sheet) => sheet.id === sheetId);
  const sheet = sheets[index];
  if (!sheet || sheet.activeSides !== "both" || (index !== 0 && index !== sheets.length - 1)) return null;
  const side = index === 0 ? "left" : "right";
  const background = sheet.visuals?.background.kind === "perSide"
    ? sheet.visuals.background[side] : null;
  const overlay = sheet.visuals?.overlay.kind === "perSide"
    ? sheet.visuals.overlay[side] : null;
  const lostBackground = background?.kind === "custom" ? background : null;
  const lostOverlay = overlay?.kind === "custom" && overlay.content !== null ? overlay : null;
  return lostBackground || lostOverlay
    ? { sheetId, sheetNumber: sheet.number, side, background: lostBackground, overlay: lostOverlay }
    : null;
}

export function albumInformationConversionLosses(
  sheets: readonly SheetSnapshot[],
  information: Readonly<AlbumInformation>,
): EdgeConversionLoss[] {
  const targets = new Set<string>();
  if (information.firstSheet === "singlePage" && sheets[0]) targets.add(sheets[0].id);
  const last = sheets[sheets.length - 1];
  if (information.lastSheet === "singlePage" && last) targets.add(last.id);
  return [...targets].flatMap((id) => {
    const loss = edgeConversionLoss(sheets, id);
    return loss ? [loss] : [];
  });
}

export function edgeConversionLossDescription(loss: EdgeConversionLoss): string {
  const subject = loss.background && loss.overlay ? "Background e Overlay personalizados"
    : loss.background ? "O Background personalizado" : "O Overlay personalizado";
  const verb = loss.background && loss.overlay ? "serão removidos" : "será removido";
  return `${subject} da página ${loss.side === "left" ? "esquerda" : "direita"} da Lâmina ${loss.sheetNumber} ${verb}.`;
}
