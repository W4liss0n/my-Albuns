import { useEffect } from "react";

import { matchProjectCommandShortcut } from "../../application/projectCommandCatalog";
import type { AlbumCanvasMode } from "../canvas/albumCanvasContract";
import { isTextEntryTarget } from "../workspace/isTextEntryTarget";

interface CanvasModeKeyboardShortcutsInput {
  implicitSheetId: string | null | undefined;
  interactionBlocked?: boolean;
  mode: AlbumCanvasMode;
  onEnterSheetEditing(sheetId: string): void;
  onExitSheetEditing(): void;
}

export function useCanvasModeKeyboardShortcuts({
  implicitSheetId,
  interactionBlocked = false,
  mode,
  onEnterSheetEditing,
  onExitSheetEditing,
}: CanvasModeKeyboardShortcutsInput) {
  useEffect(() => {
    const changeCanvasMode = (event: KeyboardEvent) => {
      // An active Canvas gesture consumes Esc and Enter to cancel itself first.
      if (event.defaultPrevented || event.repeat) return;
      if (event.key === "Escape" && mode.kind === "sheet-editing") {
        onExitSheetEditing();
        event.preventDefault();
        return;
      }
      if (
        matchProjectCommandShortcut(event, "sheet") !== "enter-sheet-editing" ||
        isTextEntryTarget(event.target)
      ) {
        return;
      }
      if (mode.kind === "sheet-editing") {
        // Like Esc, Enter leaves even while interaction is blocked.
        if (isSheetEditingFocusTarget(event.target)) {
          event.preventDefault();
          onExitSheetEditing();
        }
        return;
      }
      if (
        mode.kind === "normal" &&
        !interactionBlocked &&
        implicitSheetId &&
        isCanvasFocusTarget(event.target)
      ) {
        event.preventDefault();
        onEnterSheetEditing(implicitSheetId);
      }
    };
    window.addEventListener("keydown", changeCanvasMode);
    return () => window.removeEventListener("keydown", changeCanvasMode);
  }, [
    implicitSheetId,
    interactionBlocked,
    mode,
    onEnterSheetEditing,
    onExitSheetEditing,
  ]);
}

function isCanvasFocusTarget(target: EventTarget | null) {
  return (
    target instanceof Element && target.closest(".canvas-host") !== null
  );
}

// Controls keep their own Enter. The body counts as the Canvas, as for the
// editing zoom keys: a double click on the Sheet bar enters and removes the
// focused bar.
function isSheetEditingFocusTarget(target: EventTarget | null) {
  if (target === document.body) return true;
  return (
    isCanvasFocusTarget(target) &&
    target instanceof Element &&
    target.closest("button, [role=menu], [role=menubar]") === null
  );
}
