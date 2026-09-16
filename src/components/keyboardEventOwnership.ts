import { isTextEntryTarget } from "./isTextEntryTarget";

/** Controls and transient surfaces keep their own editing keys. */
export function ownsEditingKeys(target: EventTarget | null) {
  return isTextEntryTarget(target) || (
    target instanceof Element && target.closest(
      '[role="dialog"], [role="menu"], [role="menubar"], [role="listbox"], [role="scrollbar"]',
    ) !== null
  );
}
