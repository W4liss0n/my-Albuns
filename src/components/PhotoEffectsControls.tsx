import { Contrast } from "lucide-react";
import type { FrameSnapshot } from "../domain/project";
import { projectCommandDescriptor } from "../application/projectCommandCatalog";
import { PropertyToggle } from "../ui";

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
          Aplicado a {photos.length} {photos.length === 1 ? "foto" : "fotos"} de {frames.length} quadros
        </p>
      )}
      <PropertyToggle className="photo-effect-control"
        label={projectCommandDescriptor("toggle-photo-black-and-white").label}
        icon={Contrast}
        pressed={enabled} disabled={disabled} onToggle={onToggleBlackAndWhite} />
    </div>
  );
}
