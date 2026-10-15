import { MenuGroups, MenuItem } from "../../ui/MenuItem";
import { FRAME_STACK_COMMANDS, projectCommandDescriptor, projectCommandShortcutLabel } from "../../application/projectCommandCatalog";
import type { FrameStackAction } from "../../domain/project";
import { ContextMenuSurface } from "../../ui/ContextMenuSurface";

interface FrameContextMenuProps {
  editing?: boolean;
  hasPhoto?: boolean;
  canOpenInPhotoshop?: boolean;
  onOpenInPhotoshop?(): void;
  onShowInFolder?(): void;
  onViewPhoto?(): void;
  position: { x: number; y: number };
  onArrange(action: FrameStackAction): void;
  onDelete(): void;
  onSwapContents(): void;
  canSwapContents: boolean;
  onDismiss(): void;
}

export function FrameContextMenu({ editing = true, hasPhoto = false, canOpenInPhotoshop = false, onOpenInPhotoshop, onShowInFolder, onViewPhoto, position, onArrange, onDelete, onSwapContents, canSwapContents, onDismiss }: FrameContextMenuProps) {
  return (
    <ContextMenuSurface label={editing ? "Organizar quadros" : "Ações da foto"} position={position} onDismiss={onDismiss}>
      <MenuGroups groups={[
        [hasPhoto && onViewPhoto && <MenuItem label={projectCommandDescriptor("view-image").label} shortcut={projectCommandShortcutLabel("view-image")} onClick={() => { onViewPhoto(); onDismiss(); }} />],
        [hasPhoto && <MenuItem label={projectCommandDescriptor("open-in-photoshop").label} shortcut={projectCommandShortcutLabel("open-in-photoshop")}
          disabled={!canOpenInPhotoshop}
          onClick={() => { onOpenInPhotoshop?.(); onDismiss(); }} />,
        hasPhoto && onShowInFolder && <MenuItem label={projectCommandDescriptor("show-media-in-folder").label}
          onClick={() => { onShowInFolder(); onDismiss(); }} />],
        editing ? FRAME_STACK_COMMANDS.map(({ id, action }) => (
          <MenuItem label={projectCommandDescriptor(id).label} shortcut={projectCommandShortcutLabel(id)}
            key={id}
            onClick={() => { onArrange(action); onDismiss(); }} />
        )) : [],
        [editing && <MenuItem label={projectCommandDescriptor("swap-frame-contents").label}
          shortcut={projectCommandShortcutLabel("swap-frame-contents")}
          disabled={!canSwapContents}
          onClick={() => { onSwapContents(); onDismiss(); }} />],
        [editing && <MenuItem label={projectCommandDescriptor("delete-frames").label} shortcut={projectCommandShortcutLabel("delete-frames")}
          onClick={() => { onDelete(); onDismiss(); }} />],
      ]} />
    </ContextMenuSurface>
  );
}
