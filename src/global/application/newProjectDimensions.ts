import type {
  NewProjectConfiguration,
  ProjectConfigurationValidationCode,
  ProjectDisplayUnit,
  ProjectEndSheetFormat,
} from "./globalProjectPort";

export type PhysicalFieldName =
  | "sheetWidth"
  | "sheetHeight"
  | "bleed"
  | "safety";

export type DimensionsFieldName =
  | PhysicalFieldName
  | "dpi"
  | "sheetCount";

interface PhysicalFieldDraft {
  text: string;
  valueUm: number;
  hasExactValue: boolean;
}

export interface NewProjectDimensionsDraft {
  displayUnit: ProjectDisplayUnit;
  sheetWidth: PhysicalFieldDraft;
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
  "dpi",
  "sheetCount",
  "bleed",
  "safety",
];

const MICROMETERS_PER_UNIT: Record<ProjectDisplayUnit, bigint> = {
  mm: 1_000n,
  cm: 10_000n,
  in: 25_400n,
};

const MAX_SAFE_INTEGER = BigInt(Number.MAX_SAFE_INTEGER);
const MAX_PRESENTATION_DECIMALS = 15;

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

function physicalField(
  valueUm: number,
  unit: ProjectDisplayUnit,
): PhysicalFieldDraft {
  return {
    text: formatMicrometers(valueUm, unit),
    valueUm,
    hasExactValue: true,
  };
}

export function createDefaultDimensionsDraft(): NewProjectDimensionsDraft {
  const displayUnit = "mm";
  return {
    displayUnit,
    sheetWidth: physicalField(600_000, displayUnit),
    sheetHeight: physicalField(300_000, displayUnit),
    dpiText: "300",
    sheetCountText: "2",
    firstSheet: "double",
    lastSheet: "double",
    bleed: physicalField(3_000, displayUnit),
    safety: physicalField(3_000, displayUnit),
  };
}

export function editPhysicalField(
  draft: NewProjectDimensionsDraft,
  field: PhysicalFieldName,
  text: string,
): NewProjectDimensionsDraft {
  const parsed = parsePhysicalText(text, draft.displayUnit);
  return {
    ...draft,
    [field]: {
      text,
      valueUm: parsed ?? draft[field].valueUm,
      hasExactValue: parsed !== null,
    },
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
    sheetWidth: physicalField(draft.sheetWidth.valueUm, displayUnit),
    sheetHeight: physicalField(draft.sheetHeight.valueUm, displayUnit),
    bleed: physicalField(draft.bleed.valueUm, displayUnit),
    safety: physicalField(draft.safety.valueUm, displayUnit),
  };
}

export function formatMicrometers(
  valueUm: number,
  unit: ProjectDisplayUnit,
): string {
  return formatMicrometerFraction(valueUm, 1, unit);
}

export function getLocalInputErrors(
  draft: NewProjectDimensionsDraft,
): DimensionsErrors {
  const errors: DimensionsErrors = {};
  addPhysicalInputError(errors, "sheetWidth", draft.sheetWidth);
  addPhysicalInputError(errors, "sheetHeight", draft.sheetHeight);
  addPhysicalInputError(errors, "bleed", draft.bleed);
  addPhysicalInputError(errors, "safety", draft.safety);

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

  return {
    document: {
      displayUnit: draft.displayUnit,
      sheetWidthUm: draft.sheetWidth.valueUm,
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

export function dimensionsSummary(
  draft: NewProjectDimensionsDraft,
): string {
  const width = draft.sheetWidth.hasExactValue
    ? draft.sheetWidth.text
    : "—";
  const pageWidth = draft.sheetWidth.hasExactValue
    ? formatMicrometerFraction(
        draft.sheetWidth.valueUm,
        2,
        draft.displayUnit,
      )
    : "—";
  const height = draft.sheetHeight.hasExactValue
    ? draft.sheetHeight.text
    : "—";
  const dpi = parseIntegerText(draft.dpiText);

  return `Lâmina ${width} × ${height} ${draft.displayUnit} · Página ${pageWidth} × ${height} ${draft.displayUnit} · ${dpi ?? "—"} DPI`;
}

function formatMicrometerFraction(
  numeratorUm: number,
  divisor: number,
  unit: ProjectDisplayUnit,
): string {
  const negative = numeratorUm < 0;
  let remaining = BigInt(Math.abs(numeratorUm));
  const denominator = MICROMETERS_PER_UNIT[unit] * BigInt(divisor);
  const integer = remaining / denominator;
  remaining %= denominator;

  if (remaining === 0n) {
    return `${negative ? "-" : ""}${integer}`;
  }

  let decimals = "";
  for (
    let index = 0;
    remaining !== 0n && index < MAX_PRESENTATION_DECIMALS;
    index += 1
  ) {
    remaining *= 10n;
    decimals += (remaining / denominator).toString();
    remaining %= denominator;
  }

  return `${negative ? "-" : ""}${integer}.${decimals}`;
}

function parsePhysicalText(
  text: string,
  unit: ProjectDisplayUnit,
): number | null {
  const normalized = text.trim().replace(",", ".");
  if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/.test(normalized)) {
    return null;
  }

  const negative = normalized.startsWith("-");
  const unsigned = normalized.replace(/^[+-]/, "");
  const [whole = "0", fraction = ""] = unsigned.split(".");
  const denominator = 10n ** BigInt(fraction.length);
  const digits = `${whole || "0"}${fraction}`;
  let numerator = BigInt(digits || "0") * MICROMETERS_PER_UNIT[unit];
  if (negative) {
    numerator = -numerator;
  }
  if (numerator % denominator !== 0n) {
    return null;
  }

  const value = numerator / denominator;
  if (value > MAX_SAFE_INTEGER || value < -MAX_SAFE_INTEGER) {
    return null;
  }
  return Number(value);
}

function parseIntegerText(text: string): number | null {
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
  name: PhysicalFieldName,
  field: PhysicalFieldDraft,
) {
  if (!field.hasExactValue) {
    errors[name] = [
      "Informe uma medida decimal que corresponda a micrômetros inteiros.",
    ];
  }
}
