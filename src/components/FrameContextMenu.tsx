import { FRAME_STACK_COMMANDS, projectCommandDescriptor, projectCommandShortcutLabel } from "../application/projectCommandCatalog";
import type { FrameStackAction } from "../domain/project";
import { ContextMenuSurface } from "../ui/ContextMenuSurface";

interface FrameContextMenuProps {
  position: { x: number; y: number };
  onArrange(action: FrameStackAction): void;
  onDismiss(): void;
}

export function FrameContextMenu({ position, onArrange, onDismiss }: FrameContextMenuProps) {
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
    </ContextMenuSurface>
  );
}
