import { NumericPropertyControl, type NumericPropertyControlActions } from "./NumericPropertyControl";

export type PhotoAngleControlActions = NumericPropertyControlActions;

interface PhotoAngleControlProps extends PhotoAngleControlActions {
  value: number | null;
}

function parseAngle(text: string): number | null {
  if (!/^[+-]?\d+(?:[.,]\d)?$/.test(text.trim())) return null;
  const degrees = Number(text.trim().replace(",", "."));
  return Number.isFinite(degrees) && Math.abs(degrees) <= 45 ? Math.round(degrees * 10) : null;
}

function formatAngle(tenths: number) {
  return String(tenths / 10).replace(".", ",");
}

export function PhotoAngleControl(props: PhotoAngleControlProps) {
  return <NumericPropertyControl {...props}
    label="Ângulo" numberLabel="Ângulo em graus" sliderLabel="Ângulo da Foto" unit="°"
    minimum={-450} maximum={450} step={1} resetValue={0} sliderScale={10}
    formatValue={formatAngle} parseValue={parseAngle} valueText={(value) => `${formatAngle(value)} graus`}
    help="−45° a 45° · dois cliques para zerar" invalidHelp="Use −45° a 45°, com uma casa decimal."
  />;
}
