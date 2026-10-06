import { useEffect } from "react";

import {
  FRAME_STACK_COMMANDS,
  matchProjectCommandShortcut,
  type ProjectCommandContext,
  type ProjectCommandId,
} from "../../application/projectCommandCatalog";
import type { FrameStackAction } from "../../domain/project";
import type { ApplicationMenuCommand, ApplicationMenuGroup } from "./ApplicationMenuBar";
import { isTextEntryTarget } from "./isTextEntryTarget";
import { ownsEditingKeys } from "./keyboardEventOwnership";

const PROJECT_COMMAND_CONTEXT_ATTRIBUTE = "data-project-command-context";

function targetAllowsCommandShortcut(target: EventTarget | null, context: "sheet" | "frame" | "media-panel") {
  if (!(target instanceof Element)) return true;
  const owner = target.closest<HTMLElement>(
    `[${PROJECT_COMMAND_CONTEXT_ATTRIBUTE}]`,
  );
  return (
    owner === null ||
    owner.dataset.projectCommandContext === context
  );
}

// Clicking a thumbnail leaves the focus in the media panel; the physical
// arrows still navigate and rearrange the centered Sheet from there. The
// panel keeps Ctrl+A for its own selection.
const SHEET_ARROW_COMMANDS = new Set(["previous-sheet", "next-sheet", "next-layout", "previous-layout"]);

function sheetCommandFor(event: KeyboardEvent) {
  const command = matchProjectCommandShortcut(event, "sheet");
  if (command === null || targetAllowsCommandShortcut(event.target, "sheet")) return command;
  return SHEET_ARROW_COMMANDS.has(command) && targetAllowsCommandShortcut(event.target, "media-panel")
    ? command : null;
}

// These shortcuts run their application menu item, so a key can never act
// while the menu shows the command disabled.
const MENU_SHORTCUT_COMMANDS: ReadonlySet<ProjectCommandId> = new Set<ProjectCommandId>([
  "add-after", "duplicate-sheet", "export", "swap-frame-contents",
  "settings", "media-panel", "contextual-panel",
]);

function menuCommandFor(event: KeyboardEvent, groups: readonly ApplicationMenuGroup[]) {
  const commands = groups.flatMap((group) => group.items).flatMap<ApplicationMenuCommand>((item) =>
    item.type === "submenu" ? item.items : item.type === "command" ? [item] : []);
  return commands.find((item): item is Extract<ApplicationMenuCommand, { availability: "implemented" }> =>
    item.availability === "implemented" && item.context !== undefined &&
    MENU_SHORTCUT_COMMANDS.has(item.id as ProjectCommandId) &&
    matchProjectCommandShortcut(event, item.context as ProjectCommandContext) === item.id) ?? null;
}

interface ProjectCommandShortcutHandlers {
  newProject?(): void;
  openProject?(): void;
  selectAllFrames(): void;
  frameSelectionActive: boolean;
  openPhotoInPhotoshop?(): void;
  rotatePhotos?(): void;
  mirrorPhotos?(): void;
  togglePhotoBlackAndWhite?(): void;
  photoCommandActive?: boolean;
  importMediaFiles?(): void;
  menuGroups?: readonly ApplicationMenuGroup[];
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
  cycleToNextLayout(): void;
  cycleToPreviousLayout(): void;
  deleteSheet(): void;
  disabled: boolean;
  layoutCycleActive: boolean;
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
  newProject,
  openProject,
  selectAllFrames,
  frameSelectionActive,
  openPhotoInPhotoshop,
  rotatePhotos,
  mirrorPhotos,
  togglePhotoBlackAndWhite,
  photoCommandActive = false,
  importMediaFiles,
  menuGroups,
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
  cycleToNextLayout,
  cycleToPreviousLayout,
  deleteSheet,
  disabled,
  layoutCycleActive,
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
      if (frameSelectionActive && targetAllowsCommandShortcut(event.target, "frame") &&
          !ownsEditingKeys(event.target) && matchProjectCommandShortcut(event, "frame") === "select-all") {
        event.preventDefault();
        if (!event.repeat && !disabled) selectAllFrames();
        return;
      }
      if (photoCommandActive && targetAllowsCommandShortcut(event.target, "frame") && !ownsEditingKeys(event.target) &&
          matchProjectCommandShortcut(event, "frame-photo") === "open-in-photoshop") {
        event.preventDefault();
        if (!disabled && !event.repeat) openPhotoInPhotoshop?.();
        return;
      }
      // R, H and V have no media-panel meaning: from a thumbnail they still reach
      // the selected Frame, as right after filling it with a double-click.
      if (photoCommandActive && !ownsEditingKeys(event.target) &&
          (targetAllowsCommandShortcut(event.target, "frame") || targetAllowsCommandShortcut(event.target, "media-panel"))) {
        const photoCommand = matchProjectCommandShortcut(event, "frame-photo");
        const photoAction = photoCommand === "rotate-photo-counterclockwise" ? rotatePhotos
          : photoCommand === "mirror-photo-horizontal" ? mirrorPhotos
            : photoCommand === "toggle-photo-black-and-white" ? togglePhotoBlackAndWhite
              : undefined;
        if (photoAction) {
          event.preventDefault();
          if (!disabled && !event.repeat) photoAction();
          return;
        }
      }
      if (importMediaFiles && !ownsEditingKeys(event.target) &&
          matchProjectCommandShortcut(event, "media-panel") === "import-media-files") {
        event.preventDefault();
        if (!disabled && !event.repeat) importMediaFiles();
        return;
      }
      const menuCommand = menuGroups && !ownsEditingKeys(event.target) ? menuCommandFor(event, menuGroups) : null;
      if (menuCommand) {
        event.preventDefault();
        if (!disabled && !event.repeat && !menuCommand.disabled) menuCommand.onSelect();
        return;
      }
      if (frameClipboardActive && targetAllowsCommandShortcut(event.target, "frame") && !ownsEditingKeys(event.target)) {
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
      if (frameCommandsActive && !ownsEditingKeys(event.target)) {
        const frameCommand = matchProjectCommandShortcut(event, "frame");
        const frameOwnsTarget = targetAllowsCommandShortcut(event.target, "frame");
        // Delete has no media-panel meaning: from a thumbnail it still reaches
        // the selected Frame, as right after filling it with a double-click.
        if (frameCommand === "delete-frames" &&
            (frameOwnsTarget || targetAllowsCommandShortcut(event.target, "media-panel"))) {
          event.preventDefault();
          if (!event.repeat && !disabled) deleteFrames();
          return;
        }
        const stackCommand = frameOwnsTarget
          ? FRAME_STACK_COMMANDS.find(({ id }) => id === frameCommand)
          : undefined;
        if (stackCommand) {
          event.preventDefault();
          if (!event.repeat && !disabled) arrangeFrames(stackCommand.action);
          return;
        }
      }
      // A selected Frame keeps Delete and Enter for itself; the arrows still
      // navigate and rearrange the centered Sheet, as after filling a Frame.
      const sheetCommand = sheetCommandFor(event);
      const command =
        matchProjectCommandShortcut(event, "project-window") ??
        (sheetCommand !== null && (sheetShortcutActive || SHEET_ARROW_COMMANDS.has(sheetCommand))
          ? sheetCommand : null);
      if (command === null) return;
      if ((command === "new-project" || command === "open-project") && ownsEditingKeys(event.target)) return;
      if (command === "delete-sheet" && ownsEditingKeys(event.target)) return;
      if (
        (command === "previous-sheet" || command === "next-sheet") &&
        (!sheetNavigationActive || ownsEditingKeys(event.target))
      ) {
        return;
      }
      if (
        (command === "next-layout" || command === "previous-layout") &&
        (!layoutCycleActive || ownsEditingKeys(event.target))
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
        command === "new-project" ||
        command === "open-project" ||
        command === "save" ||
        command === "save-as" ||
        command === "close" ||
        command === "undo" ||
        command === "redo" ||
        command === "delete-sheet" ||
        command === "previous-sheet" ||
        command === "next-sheet" ||
        command === "next-layout" ||
        command === "previous-layout";
      if (!handledCommand) return;

      event.preventDefault();
      if (event.repeat || disabled) return;

      switch (command) {
        case "new-project":
          newProject?.();
          break;
        case "open-project":
          openProject?.();
          break;
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
        case "next-layout":
          cycleToNextLayout();
          break;
        case "previous-layout":
          cycleToPreviousLayout();
          break;
      }
    };
    window.addEventListener("keydown", handleProjectCommand);
    return () => window.removeEventListener("keydown", handleProjectCommand);
  }, [
    newProject,
    openProject,
    selectAllFrames,
    frameSelectionActive,
    openPhotoInPhotoshop,
    rotatePhotos,
    mirrorPhotos,
    togglePhotoBlackAndWhite,
    photoCommandActive,
    importMediaFiles,
    menuGroups,
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
    cycleToNextLayout,
    cycleToPreviousLayout,
    deleteSheet,
    disabled,
    layoutCycleActive,
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
