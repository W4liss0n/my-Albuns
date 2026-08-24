import type { ProjectConfigurationValidationError } from "../domain/generated/ProjectConfigurationValidationError";

export type ProjectConfigurationFieldName =
  | "sheetWidth"
  | "sheetHeight"
  | "bleed"
  | "safety"
  | "dpi"
  | "sheetCount";

export type ProjectConfigurationErrors = Partial<
  Record<ProjectConfigurationFieldName, readonly string[]>
>;

export const INVALID_PHYSICAL_MEASUREMENT_MESSAGE =
  "Informe uma medida decimal que corresponda a micrômetros inteiros.";

const MAX_SAFE_INTEGER = BigInt(Number.MAX_SAFE_INTEGER);
const MAX_NUMERIC_INPUT_LENGTH = 128;

const validationPresentation: Record<
  ProjectConfigurationValidationError,
  { field: ProjectConfigurationFieldName; message: string }
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

export function presentConfigurationValidationErrors(
  validationErrors: readonly ProjectConfigurationValidationError[],
): ProjectConfigurationErrors {
  const errors: ProjectConfigurationErrors = {};
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
  if (
    normalized.length > MAX_NUMERIC_INPUT_LENGTH ||
    !/^[+-]?\d+$/.test(normalized)
  ) {
    return null;
  }
  const value = BigInt(normalized);
  if (value > MAX_SAFE_INTEGER || value < -MAX_SAFE_INTEGER) {
    return null;
  }
  return Number(value);
}
