import { useCallback, useLayoutEffect, useRef, type Ref } from "react";

/**
 * Focuses a dialog's highlighted action when the dialog opens, so Enter
 * chooses it. A wrapping `DialogFocusScope` runs after this and can still
 * move the initial focus elsewhere. The returned ref also forwards the button
 * to the caller's ref.
 */
export function useDefaultDialogAction(
  focusKey: string,
  forwardedRef?: Ref<HTMLButtonElement>,
) {
  const button = useRef<HTMLButtonElement | null>(null);

  useLayoutEffect(() => {
    button.current?.focus({ preventScroll: true });
  }, [focusKey]);

  return useCallback(
    (element: HTMLButtonElement | null) => {
      button.current = element;
      if (typeof forwardedRef === "function") forwardedRef(element);
      else if (forwardedRef) forwardedRef.current = element;
    },
    [forwardedRef],
  );
}
