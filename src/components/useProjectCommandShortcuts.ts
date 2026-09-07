import { useEffect } from "react";

import { matchProjectCommandShortcut } from "../application/projectCommandCatalog";
import { isTextEntryTarget } from "./isTextEntryTarget";

const PROJECT_COMMAND_CONTEXT_ATTRIBUTE = "data-project-command-context";

function targetAllowsSheetCommandShortcut(target: EventTarget | null) {
  if (!(target instanceof Element)) return true;
  const owner = target.closest<HTMLElement>(
    `[${PROJECT_COMMAND_CONTEXT_ATTRIBUTE}]`,
  );
  return (
    owner === null ||
    owner.dataset.projectCommandContext === "sheet"
  );
}

function targetOwnsHorizontalNavigation(target: EventTarget | null) {
  if (isTextEntryTarget(target)) return true;
  return (
    target instanceof Element &&
    target.closest(
      '[role="dialog"], [role="menu"], [role="menubar"], [role="listbox"], [role="scrollbar"]',
    ) !== null
  );
}

interface ProjectCommandShortcutHandlers {
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
      const command =
        matchProjectCommandShortcut(event, "project-window") ??
        (sheetShortcutActive && targetAllowsSheetCommandShortcut(event.target)
          ? matchProjectCommandShortcut(event, "sheet")
          : null);
      if (command === null) return;
      if (
        (command === "previous-sheet" || command === "next-sheet") &&
        (!sheetNavigationActive || targetOwnsHorizontalNavigation(event.target))
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
