const AUXILIARY_BROWSER_BUTTONS = new Set([1, 3, 4]);

function eventTargetElement(target: EventTarget | null) {
  if (target instanceof Element) return target;
  if (target instanceof Node) return target.parentElement;
  return null;
}

export function installDesktopWebViewPolicy(target: Document) {
  const handleDragNavigation = (event: DragEvent) => {
    event.preventDefault();
  };
  const handleLinkNavigation = (event: MouseEvent) => {
    if (
      event.button === 0 &&
      eventTargetElement(event.target)?.closest("a[href]")
    ) {
      event.preventDefault();
    }
  };
  const handleAuxiliaryMouseAction = (event: MouseEvent) => {
    if (AUXILIARY_BROWSER_BUTTONS.has(event.button)) {
      event.preventDefault();
    }
  };

  target.addEventListener("dragover", handleDragNavigation, true);
  target.addEventListener("drop", handleDragNavigation, true);
  target.addEventListener("click", handleLinkNavigation, true);
  target.addEventListener(
    "mousedown",
    handleAuxiliaryMouseAction,
    true,
  );
  target.addEventListener(
    "auxclick",
    handleAuxiliaryMouseAction,
    true,
  );

  return () => {
    target.removeEventListener(
      "dragover",
      handleDragNavigation,
      true,
    );
    target.removeEventListener("drop", handleDragNavigation, true);
    target.removeEventListener("click", handleLinkNavigation, true);
    target.removeEventListener(
      "mousedown",
      handleAuxiliaryMouseAction,
      true,
    );
    target.removeEventListener(
      "auxclick",
      handleAuxiliaryMouseAction,
      true,
    );
  };
}
