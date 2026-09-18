import type { ProjectConfigurationValidationError } from "../domain/generated/ProjectConfigurationValidationError";
import type { ProjectConfigurationRasterLimits } from "../domain/generated/ProjectConfigurationRasterLimits";
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
  rasterLimits: ProjectConfigurationRasterLimits | null;
}

export function invalidPhysicalMeasurementMessage(unit: DisplayUnit): string {
  return `Informe uma medida válida em ${displayUnitLabel(unit)}.`;
}

const MAX_SAFE_INTEGER = BigInt(Number.MAX_SAFE_INTEGER);
const MAX_NUMERIC_INPUT_LENGTH = 128;

type ValidationMessage =
  | string
  | ((context: ProjectConfigurationValidationPresentationContext) => string);

const validationPresentation: Record<
  ProjectConfigurationValidationError,
  { field: ProjectConfigurationFieldName; message: ValidationMessage }
> = {
  sheetWidthNotPositive: {
    field: "sheetWidth",
    message: "A largura da lâmina deve ser maior que zero.",
  },
  sheetWidthAboveSafeInteger: {
    field: "sheetWidth",
    message: "A largura da lâmina excede o intervalo suportado.",
  },
  sheetWidthNotEven: {
    field: "sheetWidth",
    message:
      "A largura da lâmina precisa permitir duas páginas com a mesma medida.",
  },
  sheetWidthRasterOutOfRange: {
    field: "sheetWidth",
    message: (context) => rasterRangeMessage("width", context),
  },
  sheetHeightNotPositive: {
    field: "sheetHeight",
    message: "A altura da lâmina deve ser maior que zero.",
  },
  sheetHeightAboveSafeInteger: {
    field: "sheetHeight",
    message: "A altura da lâmina excede o intervalo suportado.",
  },
  sheetHeightRasterOutOfRange: {
    field: "sheetHeight",
    message: (context) => rasterRangeMessage("height", context),
  },
  sheetDimensionsNotProportional: {
    field: "sheetWidth",
    message:
      "A mudança de proporção deve ficar dentro do limite de 10%.",
  },
  sheetDimensionsRequireContentTransformation: {
    field: "sheetWidth",
    message:
      "Não é possível aplicar esse tamanho à composição atual.",
  },
  sheetDimensionsUnknownPhotoSize: {
    field: "sheetWidth",
    message: "Não foi possível obter o tamanho de uma foto para ajustar o recorte.",
  },
  sheetDimensionsInvalidContent: {
    field: "sheetWidth",
    message: "Esse tamanho não permite preservar todos os quadros e as medidas do álbum.",
  },
  firstSheetConversionRequiresContentReorganization: {
    field: "firstSheet",
    message:
      "Revise o conteúdo da primeira lâmina antes de alterar seu tipo.",
  },
  lastSheetConversionRequiresContentReorganization: {
    field: "lastSheet",
    message:
      "Revise o conteúdo da última lâmina antes de alterar seu tipo.",
  },
  dpiOutOfRange: {
    field: "dpi",
    message: "Use um número inteiro entre 1 e 1.200 DPI.",
  },
  sheetCountTooSmall: {
    field: "sheetCount",
    message: "O álbum deve conter pelo menos 2 lâminas.",
  },
  bleedNegative: {
    field: "bleed",
    message: "A sangria não pode ser negativa.",
  },
  bleedAboveSafeInteger: {
    field: "bleed",
    message: "A sangria excede o intervalo suportado.",
  },
  bleedEliminatesCutArea: {
    field: "bleed",
    message: "Reduza a sangria para manter uma área de corte.",
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
      "Reduza a sangria ou a margem de segurança para manter uma área útil.",
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
  const range = axis === "height"
    ? context.rasterLimits?.sheetHeight
    : context.rasterLimits?.sheetWidth;
  const dimension =
    axis === "height"
      ? "altura da lâmina"
      : context.sheetWidthPresentation === "closedSheet"
        ? "largura da lâmina fechada"
        : "largura da lâmina";
  if (!range) {
    return `A ${dimension} precisa ser ajustada para o DPI informado.`;
  }

  const divisor = axis === "width" && context.sheetWidthPresentation === "closedSheet" ? 2 : 1;
  const minimum = formatPhysicalMeasurement(
    range.minimumUm / divisor,
    context.displayUnit,
  );
  const maximum = formatPhysicalMeasurement(
    range.maximumUm / divisor,
    context.displayUnit,
  );
  const approximation =
    context.displayUnit === "in" ? "aproximadamente " : "";
  return `Para ${context.dpi} DPI, informe a ${dimension} entre ${approximation}${minimum} e ${maximum}.`;
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
