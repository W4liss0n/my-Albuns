import { Contrast } from "lucide-react";
import type { FrameSnapshot } from "../domain/project";
import { projectCommandDescriptor } from "../application/projectCommandCatalog";
import { ActionButton } from "../ui";

export interface PhotoEffectsControlActions {
  disabled: boolean;
  onToggleBlackAndWhite(): void;
}

export function PhotoEffectsControls({ frames, disabled, onToggleBlackAndWhite }:
  PhotoEffectsControlActions & { frames: readonly FrameSnapshot[] }) {
  const photos = frames.flatMap((frame) => frame.photo ? [frame.photo] : []);
  if (photos.length === 0) return null;
  const first = photos[0].transform.blackAndWhite;
  const enabled = photos.every((photo) => photo.transform.blackAndWhite === first) ? first : "mixed";
  return (
    <div className="photo-effects-controls">
      {frames.length > 1 && (
        <p className="photo-effects-scope">
          Aplicado a {photos.length} {photos.length === 1 ? "Foto" : "Fotos"} de {frames.length} Frames
        </p>
      )}
      <ActionButton className="photo-effect-control" density="compact" aria-pressed={enabled}
        disabled={disabled} onClick={onToggleBlackAndWhite}>
        <Contrast size={14} aria-hidden="true" />
        {projectCommandDescriptor("toggle-photo-black-and-white").label}
        {enabled === "mixed" && <span aria-hidden="true">—</span>}
      </ActionButton>
    </div>
  );
}
