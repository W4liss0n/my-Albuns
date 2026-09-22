import { MenuItem, MenuSeparator } from "../ui/MenuItem";
import { FRAME_STACK_COMMANDS, projectCommandDescriptor, projectCommandShortcutLabel } from "../application/projectCommandCatalog";
import type { FrameStackAction } from "../domain/project";
import { ContextMenuSurface } from "../ui/ContextMenuSurface";

interface FrameContextMenuProps {
  editing?: boolean;
  hasPhoto?: boolean;
  canOpenInPhotoshop?: boolean;
  onOpenInPhotoshop?(): void;
  onViewPhoto?(): void;
  position: { x: number; y: number };
  onArrange(action: FrameStackAction): void;
  onDelete(): void;
  onSwapContents(): void;
  canSwapContents: boolean;
  onDismiss(): void;
}

export function FrameContextMenu({ editing = true, hasPhoto = false, canOpenInPhotoshop = false, onOpenInPhotoshop, onViewPhoto, position, onArrange, onDelete, onSwapContents, canSwapContents, onDismiss }: FrameContextMenuProps) {
  return (
    <ContextMenuSurface label={editing ? "Organizar quadros" : "Ações da foto"} position={position} onDismiss={onDismiss}>
      {hasPhoto && <>
        {onViewPhoto && <MenuItem label={projectCommandDescriptor("view-image").label} shortcut={projectCommandShortcutLabel("view-image")} onClick={() => { onViewPhoto(); onDismiss(); }} />}
        <MenuItem label={projectCommandDescriptor("open-in-photoshop").label} shortcut={projectCommandShortcutLabel("open-in-photoshop")}
          disabled={!canOpenInPhotoshop}
          onClick={() => { onOpenInPhotoshop?.(); onDismiss(); }} />
        {editing && <MenuSeparator />}
      </>}
      {editing && <>
      {FRAME_STACK_COMMANDS.map(({ id, action }) => {
        const shortcut = projectCommandShortcutLabel(id);
        return (
          <MenuItem label={projectCommandDescriptor(id).label} shortcut={shortcut}
          key={id}
          onClick={() => { onArrange(action); onDismiss(); }} />
        );
      })}
      <MenuSeparator />
      <MenuItem label={projectCommandDescriptor("swap-frame-contents").label}
          disabled={!canSwapContents}
          onClick={() => { onSwapContents(); onDismiss(); }} />
      <MenuItem label={projectCommandDescriptor("delete-frames").label} shortcut={projectCommandShortcutLabel("delete-frames")}
          onClick={() => { onDelete(); onDismiss(); }} />
      </>}
    </ContextMenuSurface>
  );
}
