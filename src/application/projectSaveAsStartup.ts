export const SAVE_AS_STATE_INDETERMINATE_FRAGMENT =
  "#save-as-state-indeterminate";

export const SAVE_AS_STATE_INDETERMINATE_MESSAGE =
  "Não foi possível confirmar o destino de Salvar como. A Sessão anterior foi mantida; reinspecione o destino antes de reutilizá-lo.";

export function projectSaveAsStartupFailure(hash: string): string | null {
  return hash === SAVE_AS_STATE_INDETERMINATE_FRAGMENT
    ? SAVE_AS_STATE_INDETERMINATE_MESSAGE
    : null;
}
