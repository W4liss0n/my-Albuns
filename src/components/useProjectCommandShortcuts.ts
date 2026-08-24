import { useEffect } from "react";

import { matchProjectCommandShortcut } from "../application/projectCommandCatalog";
import { isTextEntryTarget } from "./isTextEntryTarget";

interface ProjectCommandShortcutHandlers {
  canRedo: boolean;
  canUndo: boolean;
  closeProject(): void;
  disabled: boolean;
  redo(): void;
  save(): void;
  undo(): void;
}

export function useProjectCommandShortcuts({
  canRedo,
  canUndo,
  closeProject,
  disabled,
  redo,
  save,
  undo,
}: ProjectCommandShortcutHandlers) {
  useEffect(() => {
    const handleProjectCommand = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      const command = matchProjectCommandShortcut(event, "project-window");
      if (command === null) return;
      if (
        (command === "undo" || command === "redo") &&
        isTextEntryTarget(event.target)
      ) {
        return;
      }

      const handledCommand =
        command === "save" ||
        command === "save-as" ||
        command === "close" ||
        command === "undo" ||
        command === "redo";
      if (!handledCommand) return;

      // Save as remains an explicit placeholder. Consume its accepted desktop
      // shortcut so it cannot accidentally invoke Save or browser chrome.
      event.preventDefault();
      if (event.repeat || disabled || command === "save-as") return;

      switch (command) {
        case "save":
          save();
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
      }
    };
    window.addEventListener("keydown", handleProjectCommand);
    return () => window.removeEventListener("keydown", handleProjectCommand);
  }, [canRedo, canUndo, closeProject, disabled, redo, save, undo]);
}
