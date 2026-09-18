import type { EdgeConversionLoss, SheetSnapshot } from "../domain/project";
export type { EdgeConversionLoss } from "../domain/project";

/** Facts belong to the Core projection; presentation stays with the decision. */
export function edgeConversionLoss(
  sheets: readonly SheetSnapshot[],
  sheetId: string,
): EdgeConversionLoss | null {
  return sheets.find(sheet => sheet.id === sheetId)?.edgeConversionLoss ?? null;
}

export function edgeConversionLossDescription(loss: EdgeConversionLoss): string {
  const subject = loss.background && loss.overlay ? "Fundo e sobreposição personalizados"
    : loss.background ? "O fundo personalizado" : "A sobreposição personalizada";
  const verb = loss.background && loss.overlay ? "serão removidos"
    : loss.background ? "será removido" : "será removida";
  return `${subject} da página ${loss.side === "left" ? "esquerda" : "direita"} da lâmina ${loss.sheetNumber} ${verb}.`;
}
