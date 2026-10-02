export function isTextEntryTarget(target: EventTarget | null) {
  if (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement
  ) {
    return true;
  }
  if (!(target instanceof HTMLElement)) return false;
  const contentEditableOwner = target.closest("[contenteditable]");
  return (
    target.isContentEditable ||
    (contentEditableOwner !== null &&
      contentEditableOwner.getAttribute("contenteditable") !== "false")
  );
}
