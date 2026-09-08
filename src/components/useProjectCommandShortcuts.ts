import { useEffect } from "react";

import { FRAME_STACK_COMMANDS, matchProjectCommandShortcut } from "../application/projectCommandCatalog";
import type { FrameStackAction } from "../domain/project";
import { isTextEntryTarget } from "./isTextEntryTarget";

const PROJECT_COMMAND_CONTEXT_ATTRIBUTE = "data-project-command-context";

function targetAllowsCommandShortcut(target: EventTarget | null, context: "sheet" | "frame") {
  if (!(target instanceof Element)) return true;
  const owner = target.closest<HTMLElement>(
    `[${PROJECT_COMMAND_CONTEXT_ATTRIBUTE}]`,
  );
  return (
    owner === null ||
    owner.dataset.projectCommandContext === context
  );
}

function targetOwnsEditingKeys(target: EventTarget | null) {
  if (isTextEntryTarget(target)) return true;
  return (
    target instanceof Element &&
    target.closest(
      '[role="dialog"], [role="menu"], [role="menubar"], [role="listbox"], [role="scrollbar"]',
    ) !== null
  );
}

interface ProjectCommandShortcutHandlers {
  copyFrames(): void;
  pasteFrames(): void;
  frameClipboardActive: boolean;
  arrangeFrames(action: FrameStackAction): void;
  deleteFrames(): void;
  frameCommandsActive: boolean;
  canDeleteSheet: boolean;
  canRedo: boolean;
  canUndo: boolean;
  closeProject(): void;
  deleteSheet(): void;
  disabled: boolean;
  navigateToNextSheet(): void;
  navigateToPreviousSheet(): void;
  redo(): void;
  save(): void;
  saveAs(): void;
  sheetShortcutActive: boolean;
  sheetCommandsDisabled: boolean;
  sheetNavigationActive: boolean;
  undo(): void;
}

export function useProjectCommandShortcuts({
  copyFrames,
  pasteFrames,
  frameClipboardActive,
  arrangeFrames,
  deleteFrames,
  frameCommandsActive,
  canDeleteSheet,
  canRedo,
  canUndo,
  closeProject,
  deleteSheet,
  disabled,
  navigateToNextSheet,
  navigateToPreviousSheet,
  redo,
  save,
  saveAs,
  sheetShortcutActive,
  sheetCommandsDisabled,
  sheetNavigationActive,
  undo,
}: ProjectCommandShortcutHandlers) {
  useEffect(() => {
    const handleProjectCommand = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      if (frameClipboardActive && targetAllowsCommandShortcut(event.target, "frame") && !targetOwnsEditingKeys(event.target)) {
        const command = matchProjectCommandShortcut(event, "frame");
        if (command === "copy-frames" || command === "paste-frames") {
          event.preventDefault();
          if (!event.repeat && !disabled) {
            if (command === "copy-frames") copyFrames();
            else pasteFrames();
          }
          return;
        }
      }
      if (frameCommandsActive && targetAllowsCommandShortcut(event.target, "frame") &&
          !targetOwnsEditingKeys(event.target)) {
        const frameCommand = matchProjectCommandShortcut(event, "frame");
        if (frameCommand === "delete-frames") {
          event.preventDefault();
          if (!event.repeat && !disabled) deleteFrames();
          return;
        }
        const stackCommand = FRAME_STACK_COMMANDS.find(({ id }) => id === frameCommand);
        if (stackCommand) {
          event.preventDefault();
          if (!event.repeat && !disabled) arrangeFrames(stackCommand.action);
          return;
        }
      }
      const command =
        matchProjectCommandShortcut(event, "project-window") ??
        (sheetShortcutActive && targetAllowsCommandShortcut(event.target, "sheet")
          ? matchProjectCommandShortcut(event, "sheet")
          : null);
      if (command === null) return;
      if (
        (command === "previous-sheet" || command === "next-sheet") &&
        (!sheetNavigationActive || targetOwnsEditingKeys(event.target))
      ) {
        return;
      }
      if (
        (command === "undo" ||
          command === "redo" ||
          command === "delete-sheet") &&
        isTextEntryTarget(event.target)
      ) {
        return;
      }

      const handledCommand =
        command === "save" ||
        command === "save-as" ||
        command === "close" ||
        command === "undo" ||
        command === "redo" ||
        command === "delete-sheet" ||
        command === "previous-sheet" ||
        command === "next-sheet";
      if (!handledCommand) return;

      event.preventDefault();
      if (event.repeat || disabled) return;

      switch (command) {
        case "save":
          save();
          break;
        case "save-as":
          saveAs();
          break;
        case "close":
          closeProject();
          break;
        case "undo":
          if (canUndo) undo();
          break;
        case "redo":
          if (canRedo) redo();
          break;
        case "delete-sheet":
          if (canDeleteSheet && !sheetCommandsDisabled) deleteSheet();
          break;
        case "previous-sheet":
          navigateToPreviousSheet();
          break;
        case "next-sheet":
          navigateToNextSheet();
          break;
      }
    };
    window.addEventListener("keydown", handleProjectCommand);
    return () => window.removeEventListener("keydown", handleProjectCommand);
  }, [
    copyFrames,
    pasteFrames,
    frameClipboardActive,
    arrangeFrames,
    deleteFrames,
    frameCommandsActive,
    canDeleteSheet,
    canRedo,
    canUndo,
    closeProject,
    deleteSheet,
    disabled,
    navigateToNextSheet,
    navigateToPreviousSheet,
    redo,
    save,
    saveAs,
    sheetShortcutActive,
    sheetCommandsDisabled,
    sheetNavigationActive,
    undo,
  ]);
}
