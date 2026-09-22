import { NumericPropertyControl, type NumericPropertyControlActions } from "./NumericPropertyControl";

export type PhotoZoomControlActions = NumericPropertyControlActions;

interface PhotoZoomControlProps extends PhotoZoomControlActions {
  value: number | null;
  minimum: number;
  maximum: number;
}

export function PhotoZoomControl(props: PhotoZoomControlProps) {
  return <NumericPropertyControl {...props}
    label="Zoom da foto" numberLabel="Zoom da foto em porcentagem" sliderLabel="Zoom da foto" unit="%"
    step={1} resetValue={props.minimum}
    formatValue={String} parseValue={(text) => /^\d+$/.test(text.trim()) ? Number(text) : null}
    valueText={(value) => `${value}%`}
    invalidHelp={`Use ${props.minimum}% a ${props.maximum}%.`}
  />;
}
