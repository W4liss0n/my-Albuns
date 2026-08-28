import type { ProjectConfigurationValidationError } from "../domain/generated/ProjectConfigurationValidationError";
import type { DisplayUnit } from "../domain/project";
import {
  displayUnitLabel,
  formatPhysicalMeasurement,
} from "./physicalMeasurements";

export type ProjectConfigurationFieldName =
  | "firstSheet"
  | "lastSheet"
  | "sheetWidth"
  | "sheetHeight"
  | "bleed"
  | "safety"
  | "dpi"
  | "sheetCount";

export type ProjectConfigurationErrors = Partial<
  Record<ProjectConfigurationFieldName, readonly string[]>
>;

export interface ProjectConfigurationValidationPresentationContext {
  displayUnit: DisplayUnit;
  dpi: number;
  sheetWidthPresentation: "openSheet" | "closedSheet";
}

export function invalidPhysicalMeasurementMessage(unit: DisplayUnit): string {
  return `Informe uma medida válida em ${displayUnitLabel(unit)}.`;
}

const MAX_SAFE_INTEGER = BigInt(Number.MAX_SAFE_INTEGER);
const MAX_NUMERIC_INPUT_LENGTH = 128;
const MICROMETERS_PER_INCH = 25_400n;
const RASTER_ROUNDING_OFFSET = 12_700n;
const MAX_RASTER_AXIS = 65_535n;

type ValidationMessage =
  | string
  | ((context: ProjectConfigurationValidationPresentationContext) => string);

const validationPresentation: Record<
  ProjectConfigurationValidationError,
  { field: ProjectConfigurationFieldName; message: ValidationMessage }
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
    message:
      "A largura da Lâmina precisa permitir duas Páginas com a mesma medida.",
  },
  sheetWidthRasterOutOfRange: {
    field: "sheetWidth",
    message: (context) => rasterRangeMessage("width", context),
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
    message: (context) => rasterRangeMessage("height", context),
  },
  sheetDimensionsNotProportional: {
    field: "sheetWidth",
    message:
      "Mantenha a proporção atual da Lâmina para preservar a composição.",
  },
  sheetDimensionsRequireContentTransformation: {
    field: "sheetWidth",
    message:
      "A composição existente exige o fluxo de mudança dimensional segura.",
  },
  firstSheetConversionRequiresContentReorganization: {
    field: "firstSheet",
    message:
      "A primeira Lâmina contém composição e exige o fluxo completo de conversão.",
  },
  lastSheetConversionRequiresContentReorganization: {
    field: "lastSheet",
    message:
      "A última Lâmina contém composição e exige o fluxo completo de conversão.",
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
  context: ProjectConfigurationValidationPresentationContext,
): ProjectConfigurationErrors {
  const errors: ProjectConfigurationErrors = {};
  for (const error of validationErrors) {
    const presentation = validationPresentation[error];
    const message =
      typeof presentation.message === "function"
        ? presentation.message(context)
        : presentation.message;
    errors[presentation.field] = [
      ...(errors[presentation.field] ?? []),
      message,
    ];
  }
  return errors;
}

function rasterRangeMessage(
  axis: "width" | "height",
  context: ProjectConfigurationValidationPresentationContext,
): string {
  const rangeKind =
    axis === "height" ? "height" : context.sheetWidthPresentation;
  const range = physicalRasterRange(context.dpi, rangeKind);
  const dimension =
    axis === "height"
      ? "altura da Lâmina"
      : context.sheetWidthPresentation === "closedSheet"
        ? "largura da Lâmina fechada"
        : "largura da Lâmina";
  if (!range) {
    return `A ${dimension} precisa ser ajustada para o DPI informado.`;
  }

  const minimum = formatPhysicalMeasurement(
    range.minimumUm,
    context.displayUnit,
  );
  const maximum = formatPhysicalMeasurement(
    range.maximumUm,
    context.displayUnit,
  );
  const approximation =
    context.displayUnit === "in" ? "aproximadamente " : "";
  return `Para ${context.dpi} DPI, informe a ${dimension} entre ${approximation}${minimum} e ${maximum}.`;
}

function physicalRasterRange(
  dpi: number,
  kind: "height" | "openSheet" | "closedSheet",
): { minimumUm: number; maximumUm: number } | null {
  if (!Number.isSafeInteger(dpi) || dpi <= 0) return null;

  const dpiValue = BigInt(dpi);
  const minimumAxisUm =
    (RASTER_ROUNDING_OFFSET + dpiValue - 1n) / dpiValue;
  const maximumAxisUm =
    ((MAX_RASTER_AXIS + 1n) * MICROMETERS_PER_INCH -
      1n -
      RASTER_ROUNDING_OFFSET) /
    dpiValue;
  let minimumUm = minimumAxisUm;
  let maximumUm = maximumAxisUm;
  if (kind === "openSheet") {
    minimumUm *= 2n;
    maximumUm -= maximumUm % 2n;
  } else if (kind === "closedSheet") {
    maximumUm /= 2n;
  }

  return {
    minimumUm: Number(minimumUm),
    maximumUm: Number(maximumUm),
  };
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
