import { FlipHorizontal2, RotateCcw } from "lucide-react";
import type { FrameSnapshot, PhotoOrientationAction } from "../domain/project";
import { projectCommandDescriptor } from "../application/projectCommandCatalog";
import { ActionButton, AppIcon, PropertyToggle } from "../ui";
import { PhotoAngleControl, type PhotoAngleControlActions } from "./PhotoAngleControl";

export interface PhotoOrientationControlActions {
  disabled: boolean;
  onAction(action: PhotoOrientationAction): void;
  angle?: PhotoAngleControlActions;
}

interface PhotoOrientationControlsProps extends PhotoOrientationControlActions {
  frames: readonly FrameSnapshot[];
}

export function PhotoOrientationControls({ frames, disabled, onAction, angle }: PhotoOrientationControlsProps) {
  const photos = frames.flatMap((frame) => frame.photo ? [frame.photo] : []);
  if (photos.length === 0) return null;
  const first = photos[0].transform;
  const turns = photos.every((photo) => photo.transform.quarterTurns === first.quarterTurns)
    ? first.quarterTurns : null;
  const mirrored = photos.every((photo) => photo.transform.mirrorX === first.mirrorX)
    ? first.mirrorX : "mixed";
  const angleTenths = photos.every((photo) => photo.transform.fineRotationDegrees === first.fineRotationDegrees)
    ? Math.round(first.fineRotationDegrees * 10) : null;
  const rotate = projectCommandDescriptor("rotate-photo-counterclockwise");
  const mirror = projectCommandDescriptor("mirror-photo-horizontal");
  return (
    <div className="photo-orientation-controls">
      {frames.length > 1 && (
        <p className="photo-orientation-scope">
          Aplicado a {photos.length} {photos.length === 1 ? "foto" : "fotos"} de {frames.length} quadros
        </p>
      )}
      <div className="photo-orientation-row">
        <div className="photo-orientation-label">
          <span>Giro</span>
          <output
            className="photo-rotation-readout"
            aria-label="Giro das fotos"
          >
            {turns === null ? "—" : `${((4 - turns) % 4) * 90}°`}
          </output>
        </div>
        <div className="photo-orientation-actions" role="group" aria-label="Orientação da foto">
          <ActionButton
            density="compact"
            variant="integrated"
            aria-label={rotate.label}
            title={rotate.label}
            disabled={disabled}
            onClick={() => onAction("rotateCounterClockwise")}
          >
            <AppIcon icon={RotateCcw} size={16} /> 90°
          </ActionButton>
          <PropertyToggle
            className="photo-mirror-control"
            label={mirror.label}
            icon={FlipHorizontal2}
            pressed={mirrored}
            disabled={disabled}
            onToggle={() => onAction("toggleHorizontalMirror")}
            tooltipSide="left"
          />
        </div>
      </div>
      {angle && <PhotoAngleControl key={angle.scopeKey} value={angleTenths} {...angle} />}
    </div>
  );
}
