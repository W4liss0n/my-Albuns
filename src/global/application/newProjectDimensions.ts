import type {
  NewProjectConfiguration,
  ProjectConfigurationValidationCode,
  ProjectDisplayUnit,
  ProjectEndSheetFormat,
} from "./globalProjectPort";
import {
  createPhysicalFieldDraft,
  editPhysicalFieldDraft,
  type PhysicalFieldDraft,
} from "../../application/physicalMeasurements";

export {
  displayUnitLabel,
  formatMicrometers,
  parsePhysicalText,
} from "../../application/physicalMeasurements";
export type { PhysicalFieldDraft } from "../../application/physicalMeasurements";

export type PhysicalFieldName =
  | "closedSheetWidth"
  | "sheetHeight"
  | "bleed"
  | "safety";

export type DimensionsFieldName =
  | "sheetWidth"
  | "sheetHeight"
  | "bleed"
  | "safety"
  | "dpi"
  | "sheetCount";

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

export type DimensionsErrors = Partial<
  Record<DimensionsFieldName, readonly string[]>
>;

export const DIMENSIONS_FIELD_ORDER: readonly DimensionsFieldName[] = [
  "sheetWidth",
  "sheetHeight",
  "bleed",
  "safety",
  "sheetCount",
  "dpi",
];

const MAX_SAFE_INTEGER = BigInt(Number.MAX_SAFE_INTEGER);

const validationPresentation: Record<
  ProjectConfigurationValidationCode,
  { field: DimensionsFieldName; message: string }
> = {
  sheetWidthNotPositive: {
    field: "sheetWidth",
    message: "A largura da Lâmina deve ser maior que zero.",
  },
  sheetWidthAboveSafeInteger: {
    field: "sheetWidth",
    message: "A largura da Lâmina excede o intervalo suportado.",
  },
  sheetWidthNotEven: {
    field: "sheetWidth",
    message: "A largura da Lâmina precisa resultar em micrômetros pares.",
  },
  sheetWidthRasterOutOfRange: {
    field: "sheetWidth",
    message:
      "Lâmina e Página devem ter largura raster entre 1 e 65.535 pixels.",
  },
  sheetHeightNotPositive: {
    field: "sheetHeight",
    message: "A altura da Lâmina deve ser maior que zero.",
  },
  sheetHeightAboveSafeInteger: {
    field: "sheetHeight",
    message: "A altura da Lâmina excede o intervalo suportado.",
  },
  sheetHeightRasterOutOfRange: {
    field: "sheetHeight",
    message: "A altura raster deve ficar entre 1 e 65.535 pixels.",
  },
  sheetDimensionsNotProportional: {
    field: "sheetWidth",
    message:
      "Mantenha a proporção atual da Lâmina para preservar a composição.",
  },
  dpiOutOfRange: {
    field: "dpi",
    message: "Informe um DPI inteiro entre 1 e 1.200.",
  },
  sheetCountTooSmall: {
    field: "sheetCount",
    message: "O Álbum deve conter pelo menos 2 Lâminas.",
  },
  bleedNegative: {
    field: "bleed",
    message: "A Sangria não pode ser negativa.",
  },
  bleedAboveSafeInteger: {
    field: "bleed",
    message: "A Sangria excede o intervalo suportado.",
  },
  bleedEliminatesCutArea: {
    field: "bleed",
    message: "A Sangria deve manter uma Área de corte positiva.",
  },
  safetyNegative: {
    field: "safety",
    message: "A segurança não pode ser negativa.",
  },
  safetyAboveSafeInteger: {
    field: "safety",
    message: "A segurança excede o intervalo suportado.",
  },
  safetyEliminatesSafeArea: {
    field: "safety",
    message:
      "Sangria e segurança devem manter uma Área de segurança positiva.",
  },
};

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
): DimensionsErrors {
  const errors: DimensionsErrors = {};
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

export function presentConfigurationValidationErrors(
  validationErrors: readonly ProjectConfigurationValidationCode[],
): DimensionsErrors {
  const errors: DimensionsErrors = {};
  for (const error of validationErrors) {
    const presentation = validationPresentation[error];
    errors[presentation.field] = [
      ...(errors[presentation.field] ?? []),
      presentation.message,
    ];
  }
  return errors;
}

export function parseIntegerText(text: string): number | null {
  const normalized = text.trim();
  if (!/^[+-]?\d+$/.test(normalized)) {
    return null;
  }
  const value = BigInt(normalized);
  if (value > MAX_SAFE_INTEGER || value < -MAX_SAFE_INTEGER) {
    return null;
  }
  return Number(value);
}

function addPhysicalInputError(
  errors: DimensionsErrors,
  name: DimensionsFieldName,
  field: PhysicalFieldDraft,
) {
  if (!field.hasExactValue) {
    errors[name] = [
      "Informe uma medida decimal que corresponda a micrômetros inteiros.",
    ];
  }
}
