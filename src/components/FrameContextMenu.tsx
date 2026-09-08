import { FRAME_STACK_COMMANDS, projectCommandDescriptor, projectCommandShortcutLabel } from "../application/projectCommandCatalog";
import type { FrameStackAction } from "../domain/project";
import { ContextMenuSurface } from "../ui/ContextMenuSurface";

interface FrameContextMenuProps {
  position: { x: number; y: number };
  onArrange(action: FrameStackAction): void;
  onDelete(): void;
  onSwapContents(): void;
  canSwapContents: boolean;
  onDismiss(): void;
}

export function FrameContextMenu({ position, onArrange, onDelete, onSwapContents, canSwapContents, onDismiss }: FrameContextMenuProps) {
  return (
    <ContextMenuSurface label="Organizar Frames" position={position} onDismiss={onDismiss}>
      {FRAME_STACK_COMMANDS.map(({ id, action }) => {
        const shortcut = projectCommandShortcutLabel(id);
        return (
          <button key={id} type="button" role="menuitem"
            onClick={() => { onArrange(action); onDismiss(); }}>
            <span>{projectCommandDescriptor(id).label}</span>
            {shortcut ? <kbd aria-hidden="true">{shortcut}</kbd> : null}
          </button>
        );
      })}
      <div className="ui-context-menu__separator" role="separator" />
      <button type="button" role="menuitem" disabled={!canSwapContents}
        onClick={() => { onSwapContents(); onDismiss(); }}>
        {projectCommandDescriptor("swap-frame-contents").label}
      </button>
      <button type="button" role="menuitem" aria-label={projectCommandDescriptor("delete-frames").label}
        onClick={() => { onDelete(); onDismiss(); }}>
        <span>{projectCommandDescriptor("delete-frames").label}</span>
        <kbd aria-hidden="true">{projectCommandShortcutLabel("delete-frames")}</kbd>
      </button>
    </ContextMenuSurface>
  );
}
