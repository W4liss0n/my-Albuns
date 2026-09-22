export type VisualSelectionValue =
  | { kind: "color"; rgb: string }
  | { kind: "media"; mediaId: string }
  | { kind: "none" };

/** Presentation ignores origin: equal inherited/custom colors are still one color. */
export function summarizeVisualSelection(values: readonly VisualSelectionValue[]) {
  const first = values[0];
  const uniform = first !== undefined && values.every(value => {
    if (first.kind === "color") return value.kind === "color" && value.rgb.toUpperCase() === first.rgb.toUpperCase();
    if (first.kind === "media") return value.kind === "media" && value.mediaId === first.mediaId;
    return value.kind === "none";
  });
  return {
    mixed: !uniform,
    rgb: uniform && first.kind === "color" ? first.rgb : null,
    mediaId: uniform && first.kind === "media" ? first.mediaId : null,
    none: uniform && first.kind === "none",
    previewColors: !uniform && values.length === 2 && values[0].kind === "color" && values[1].kind === "color"
      ? [values[0].rgb, values[1].rgb] as const : undefined,
  };
}
