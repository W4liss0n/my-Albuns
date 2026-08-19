import { useEffect } from "react";

import type { AlbumCanvasMode } from "./albumCanvasContract";

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
      if (event.defaultPrevented || event.repeat) return;
      if (event.key === "Escape" && mode.kind === "sheet-editing") {
        onExitSheetEditing();
        event.preventDefault();
        return;
      }
      if (
        event.key === "Enter" &&
        mode.kind === "normal" &&
        !interactionBlocked &&
        implicitSheetId &&
        !isTextEntryTarget(event.target) &&
        isCanvasFocusTarget(event.target)
      ) {
        onEnterSheetEditing(implicitSheetId);
        event.preventDefault();
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

function isTextEntryTarget(target: EventTarget | null) {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

function isCanvasFocusTarget(target: EventTarget | null) {
  return (
    target instanceof Element && target.closest(".canvas-host") !== null
  );
}
