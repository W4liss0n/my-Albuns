import type {
  NewProjectConfiguration,
  ProjectDisplayUnit,
  ProjectEndSheetFormat,
} from "./globalProjectPort";
import {
  INVALID_PHYSICAL_MEASUREMENT_MESSAGE,
  parseIntegerText,
  type ProjectConfigurationErrors,
  type ProjectConfigurationFieldName,
} from "../../application/projectConfigurationFields";
import {
  createPhysicalFieldDraft,
  editPhysicalFieldDraft,
  type PhysicalFieldDraft,
} from "../../application/physicalMeasurements";

export type PhysicalFieldName =
  | "closedSheetWidth"
  | "sheetHeight"
  | "bleed"
  | "safety";

export interface NewProjectDimensionsDraft {
  displayUnit: ProjectDisplayUnit;
  closedSheetWidth: PhysicalFieldDraft;
  sheetHeight: PhysicalFieldDraft;
  dpiText: string;
  sheetCountText: string;
  firstSheet: ProjectEndSheetFormat;
  lastSheet: ProjectEndSheetFormat;
  bleed: PhysicalFieldDraft;
  safety: PhysicalFieldDraft;
}

export const DIMENSIONS_FIELD_ORDER: readonly ProjectConfigurationFieldName[] = [
  "sheetWidth",
  "sheetHeight",
  "bleed",
  "safety",
  "sheetCount",
  "dpi",
];

export function createDefaultDimensionsDraft(): NewProjectDimensionsDraft {
  const displayUnit = "mm";
  return {
    displayUnit,
    closedSheetWidth: createPhysicalFieldDraft(300_000, displayUnit),
    sheetHeight: createPhysicalFieldDraft(300_000, displayUnit),
    dpiText: "300",
    sheetCountText: "18",
    firstSheet: "double",
    lastSheet: "double",
    bleed: createPhysicalFieldDraft(3_000, displayUnit),
    safety: createPhysicalFieldDraft(5_000, displayUnit),
  };
}

export function editPhysicalField(
  draft: NewProjectDimensionsDraft,
  field: PhysicalFieldName,
  text: string,
): NewProjectDimensionsDraft {
  return {
    ...draft,
    [field]: editPhysicalFieldDraft(
      draft[field],
      text,
      draft.displayUnit,
    ),
  };
}

export function changeDisplayUnit(
  draft: NewProjectDimensionsDraft,
  displayUnit: ProjectDisplayUnit,
): NewProjectDimensionsDraft {
  if (displayUnit === draft.displayUnit) {
    return draft;
  }

  return {
    ...draft,
    displayUnit,
    closedSheetWidth: createPhysicalFieldDraft(
      draft.closedSheetWidth.valueUm,
      displayUnit,
    ),
    sheetHeight: createPhysicalFieldDraft(
      draft.sheetHeight.valueUm,
      displayUnit,
    ),
    bleed: createPhysicalFieldDraft(draft.bleed.valueUm, displayUnit),
    safety: createPhysicalFieldDraft(draft.safety.valueUm, displayUnit),
  };
}

export function getLocalInputErrors(
  draft: NewProjectDimensionsDraft,
): ProjectConfigurationErrors {
  const errors: ProjectConfigurationErrors = {};
  addPhysicalInputError(errors, "sheetWidth", draft.closedSheetWidth);
  addPhysicalInputError(errors, "sheetHeight", draft.sheetHeight);
  addPhysicalInputError(errors, "bleed", draft.bleed);
  addPhysicalInputError(errors, "safety", draft.safety);
  if (
    draft.closedSheetWidth.hasExactValue &&
    !Number.isSafeInteger(draft.closedSheetWidth.valueUm * 2)
  ) {
    errors.sheetWidth = [
      "A largura aberta da Lâmina excede o intervalo suportado.",
    ];
  }

  if (parseIntegerText(draft.dpiText) === null) {
    errors.dpi = ["Informe o DPI como um número inteiro suportado."];
  }
  if (parseIntegerText(draft.sheetCountText) === null) {
    errors.sheetCount = [
      "Informe a quantidade como um número inteiro suportado.",
    ];
  }
  return errors;
}

export function toNewProjectConfiguration(
  draft: NewProjectDimensionsDraft,
): NewProjectConfiguration | null {
  if (Object.keys(getLocalInputErrors(draft)).length > 0) {
    return null;
  }

  const dpi = parseIntegerText(draft.dpiText);
  const sheetCount = parseIntegerText(draft.sheetCountText);
  if (dpi === null || sheetCount === null) {
    return null;
  }

  const sheetWidthUm = draft.closedSheetWidth.valueUm * 2;

  return {
    document: {
      displayUnit: draft.displayUnit,
      sheetWidthUm,
      sheetHeightUm: draft.sheetHeight.valueUm,
      dpi,
      bleedUm: draft.bleed.valueUm,
      safetyUm: draft.safety.valueUm,
    },
    structure: {
      sheetCount,
      firstSheet: draft.firstSheet,
      lastSheet: draft.lastSheet,
    },
  };
}

function addPhysicalInputError(
  errors: ProjectConfigurationErrors,
  name: ProjectConfigurationFieldName,
  field: PhysicalFieldDraft,
) {
  if (!field.hasExactValue) {
    errors[name] = [INVALID_PHYSICAL_MEASUREMENT_MESSAGE];
  }
}
