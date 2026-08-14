export interface DialogAction {
  disabled?: boolean;
  label: string;
  onClick(): void;
}
