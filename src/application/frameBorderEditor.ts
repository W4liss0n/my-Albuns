export interface SolidFrameBorder {
  rgb: string;
  widthUm: number;
}

export type FrameBorderValue =
  | { kind: "none" }
  | ({ kind: "solid" } & SolidFrameBorder);

export interface FrameBorderEditorState {
  border: FrameBorderValue;
  solid: SolidFrameBorder;
}

export function createFrameBorderEditorState(
  border: FrameBorderValue,
  fallback: SolidFrameBorder,
): FrameBorderEditorState {
  const solid = border.kind === "solid" ? border : normalizeSolid(fallback);
  return { border, solid };
}

export function changeFrameBorderWidth(
  state: FrameBorderEditorState,
  widthUm: number,
): FrameBorderEditorState {
  if (!Number.isFinite(widthUm)) return state;
  if (widthUm <= 0) {
    return { border: { kind: "none" }, solid: state.solid };
  }
  const solid = { ...state.solid, widthUm: Math.max(1, Math.trunc(widthUm)) };
  return { border: { kind: "solid", ...solid }, solid };
}

export function changeFrameBorderColor(
  state: FrameBorderEditorState,
  rgb: string,
): FrameBorderEditorState {
  const solid = { ...state.solid, rgb: rgb.toUpperCase() };
  return {
    border:
      state.border.kind === "solid"
        ? { kind: "solid", ...solid }
        : state.border,
    solid,
  };
}

function normalizeSolid(solid: SolidFrameBorder): SolidFrameBorder {
  return {
    rgb: solid.rgb.toUpperCase(),
    widthUm: Math.max(1, Math.trunc(solid.widthUm)),
  };
}
