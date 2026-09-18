export const SAVE_AS_STATE_INDETERMINATE_FRAGMENT =
  "#save-as-state-indeterminate";

export const SAVE_AS_STATE_INDETERMINATE_MESSAGE =
  "Não foi possível confirmar se a cópia foi salva. O projeto anterior continua aberto. Confira o arquivo no destino escolhido antes de tentar novamente.";

export function projectSaveAsStartupFailure(hash: string): string | null {
  return hash === SAVE_AS_STATE_INDETERMINATE_FRAGMENT
    ? SAVE_AS_STATE_INDETERMINATE_MESSAGE
    : null;
}
