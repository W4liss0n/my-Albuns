/**
 * A right click while a drag holds the primary button neither opens a context
 * menu nor ends the drag. Call it when the press starts; the returned function
 * lifts the block after the current event, because Windows sends `contextmenu`
 * right after the release that may end the drag.
 */
export function blockContextMenuDuringGesture(): () => void {
  const block = (event: MouseEvent) => {
    event.preventDefault();
    event.stopImmediatePropagation();
  };
  window.addEventListener("contextmenu", block, true);
  let lifted = false;
  return () => {
    if (lifted) return;
    lifted = true;
    window.setTimeout(
      () => window.removeEventListener("contextmenu", block, true),
      0,
    );
  };
}
