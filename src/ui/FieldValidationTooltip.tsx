export interface FieldValidationTooltipModel {
  id: string;
  message: string;
}

export function createFieldValidationTooltip(
  id: string,
  messages: readonly (string | undefined)[],
): FieldValidationTooltipModel {
  return {
    id,
    message: [
      ...new Set(
        messages.filter((message): message is string => Boolean(message)),
      ),
    ].join("\n"),
  };
}

export function fieldValidationTooltipAttributes(
  error: string | undefined,
  tooltip: FieldValidationTooltipModel,
) {
  const invalid = Boolean(error && tooltip.message);
  return {
    "aria-describedby": invalid ? tooltip.id : undefined,
    "aria-invalid": invalid || undefined,
    title: invalid ? tooltip.message : undefined,
  };
}

export function FieldValidationTooltip({
  tooltip,
}: {
  tooltip: FieldValidationTooltipModel;
}) {
  return tooltip.message ? (
    <p className="ui-visually-hidden" id={tooltip.id} role="alert">
      {tooltip.message}
    </p>
  ) : null;
}
