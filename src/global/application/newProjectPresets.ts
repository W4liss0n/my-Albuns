import {
  createDefaultDimensionsDraft,
  editPhysicalField,
  type NewProjectDimensionsDraft,
} from "./newProjectDimensions";
import {
  createDefaultPersonalizationDraft,
  setBackgroundColor,
  setFrameBorderColor,
  setFrameBorderEnabled,
  setFrameBorderWidth,
  type NewProjectPersonalizationDraft,
} from "./newProjectPersonalization";

/**
 * PLACEHOLDER UI: predefinições ainda não possuem contrato nem persistência no
 * backend. Este modelo existe somente para reproduzir e validar o fluxo visual
 * da referência durante a sessão atual. Substitua-o por uma porta de aplicação
 * antes de tratar uma predefinição como dado durável do produto.
 */
export interface NewProjectPreset {
  id: string;
  name: string;
  dimensions: NewProjectDimensionsDraft;
  personalization: NewProjectPersonalizationDraft;
}

export function createBuiltInProjectPresets(): readonly NewProjectPreset[] {
  return [
    projectPreset({
      id: "builtin-graphic-30",
      name: "Gráfica 30 × 30",
      closedWidthMm: "300",
      heightMm: "300",
      bleedMm: "3",
      safetyMm: "5",
      sheetCount: 18,
      background: "#F7F5F0",
    }),
    projectPreset({
      id: "builtin-fine-art-25",
      name: "Fine art 25 × 25",
      closedWidthMm: "250",
      heightMm: "250",
      bleedMm: "5",
      safetyMm: "8",
      sheetCount: 24,
      background: "#EEE6D8",
      border: { rgb: "#FFFFFF", widthUm: 4_000 },
    }),
    projectPreset({
      id: "builtin-book-20-30",
      name: "Book 20 × 30",
      closedWidthMm: "200",
      heightMm: "300",
      bleedMm: "3",
      safetyMm: "6",
      sheetCount: 12,
      background: "#FFFFFF",
      border: { rgb: "#2C2924", widthUm: 1_000 },
    }),
  ];
}

function projectPreset({
  background,
  bleedMm,
  border,
  closedWidthMm,
  heightMm,
  id,
  name,
  safetyMm,
  sheetCount,
}: {
  background: string;
  bleedMm: string;
  border?: { rgb: string; widthUm: number };
  closedWidthMm: string;
  heightMm: string;
  id: string;
  name: string;
  safetyMm: string;
  sheetCount: number;
}): NewProjectPreset {
  let dimensions = createDefaultDimensionsDraft();
  dimensions = editPhysicalField(
    dimensions,
    "closedSheetWidth",
    closedWidthMm,
  );
  dimensions = editPhysicalField(dimensions, "sheetHeight", heightMm);
  dimensions = editPhysicalField(dimensions, "bleed", bleedMm);
  dimensions = editPhysicalField(dimensions, "safety", safetyMm);
  dimensions = { ...dimensions, sheetCountText: String(sheetCount) };

  let personalization = setBackgroundColor(
    createDefaultPersonalizationDraft(),
    background,
  );
  if (border) {
    personalization = setFrameBorderEnabled(personalization, true);
    personalization = setFrameBorderColor(personalization, border.rgb);
    personalization = setFrameBorderWidth(personalization, border.widthUm);
  }

  return { id, name, dimensions, personalization };
}
