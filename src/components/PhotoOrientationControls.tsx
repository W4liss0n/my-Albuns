import { FlipHorizontal2, RotateCcw } from "lucide-react";
import type { FrameSnapshot, PhotoOrientationAction } from "../domain/project";
import { projectCommandDescriptor } from "../application/projectCommandCatalog";
import { ActionButton } from "../ui";

export interface PhotoOrientationControlActions {
  disabled: boolean;
  onAction(action: PhotoOrientationAction): void;
}

interface PhotoOrientationControlsProps extends PhotoOrientationControlActions {
  frames: readonly FrameSnapshot[];
}

export function PhotoOrientationControls({ frames, disabled, onAction }: PhotoOrientationControlsProps) {
  const photos = frames.flatMap((frame) => frame.photo ? [frame.photo] : []);
  if (photos.length === 0) return null;
  const first = photos[0].transform;
  const turns = photos.every((photo) => photo.transform.quarterTurns === first.quarterTurns)
    ? first.quarterTurns : null;
  const mirrored = photos.every((photo) => photo.transform.mirrorX === first.mirrorX)
    ? first.mirrorX : "mixed";
  const rotate = projectCommandDescriptor("rotate-photo-counterclockwise");
  const reset = projectCommandDescriptor("reset-photo-rotation");
  const mirror = projectCommandDescriptor("mirror-photo-horizontal");
  return (
    <div className="photo-orientation-controls">
      {frames.length > 1 && (
        <p className="photo-orientation-scope">
          Aplicado a {photos.length} {photos.length === 1 ? "Foto" : "Fotos"} de {frames.length} Frames
        </p>
      )}
      <div className="photo-orientation-row">
        <span>Giro</span>
        <output aria-label="Giro das Fotos">{turns === null ? "—" : `${((4 - turns) % 4) * 90}°`}</output>
        <ActionButton
          density="compact"
          aria-label={rotate.label}
          title={rotate.label}
          disabled={disabled}
          onClick={() => onAction("rotateCounterClockwise")}
        >
          <RotateCcw size={14} aria-hidden="true" /> 90°
        </ActionButton>
        <ActionButton
          density="compact"
          aria-label={reset.label}
          title={reset.label}
          disabled={disabled || turns === 0}
          onClick={() => onAction("resetRotation")}
        >
          0°
        </ActionButton>
      </div>
      <ActionButton
        className="photo-mirror-control"
        density="compact"
        aria-pressed={mirrored}
        disabled={disabled}
        onClick={() => onAction("toggleHorizontalMirror")}
      >
        <FlipHorizontal2 size={14} aria-hidden="true" /> {mirror.label}
        {mirrored === "mixed" && <span aria-hidden="true">—</span>}
      </ActionButton>
    </div>
  );
}
