export const FOCUSED_OWNED_DIALOG_SCENARIOS = Object.freeze([
  "external-copy-opening-owner",
  "late-graphics-project-dialog",
]);

export function selectFocusedOwnedDialogScenarios(selection = "all") {
  if (selection === "all") return [...FOCUSED_OWNED_DIALOG_SCENARIOS];
  if (!FOCUSED_OWNED_DIALOG_SCENARIOS.includes(selection)) {
    throw new Error(`Unknown focused native scenario: ${selection}`);
  }
  return [selection];
}
