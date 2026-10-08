import { matchProjectCommandShortcut } from "../../application/projectCommandCatalog";

/**
 * Esc and the Enter that toggles Sheet editing only cancel an active Canvas
 * gesture. The gesture consumes them, so the same press never changes the mode.
 */
export function isGestureCancelKey(event: KeyboardEvent) {
  return event.key === "Escape" ||
    matchProjectCommandShortcut(event, "sheet") === "enter-sheet-editing";
}
