import type {
  Matrix2,
  NumberRange,
  PhotoPlacement,
  PhotoPlacementPlan,
  Vector2,
} from "../domain/project";

export interface ConstrainedPhotoPlacement {
  pan: Vector2;
  placement: PhotoPlacement;
}

export interface ZoomedPhotoPlacement {
  zoom: number;
  placement: PhotoPlacement;
}

export interface PhotoGeometry {
  current: PhotoPlacement;
  panRange: NumberRange;
  zoomRange: NumberRange;
  zoom(targetZoom: number): ZoomedPhotoPlacement;
  constrain(center: Vector2): ConstrainedPhotoPlacement;
}

export function createPhotoGeometry(
  sourcePlan: PhotoPlacementPlan,
  unitScale = 1,
): PhotoGeometry {
  const plan = scalePlan(sourcePlan, unitScale);

  function zoom(targetZoom: number): ZoomedPhotoPlacement {
    const boundedZoom = clampToRange(targetZoom, plan.zoomRange);
    const delta = boundedZoom - plan.currentZoom;
    return {
      zoom: boundedZoom,
      placement: {
        center: {
          x: plan.current.center.x + plan.centerPerZoom.x * delta,
          y: plan.current.center.y + plan.centerPerZoom.y * delta,
        },
        size: {
          width:
            plan.current.size.width + plan.sizePerZoom.width * delta,
          height:
            plan.current.size.height + plan.sizePerZoom.height * delta,
        },
      },
    };
  }

  function constrain(center: Vector2): ConstrainedPhotoPlacement {
    const offset = {
      x: center.x - plan.panOrigin.x,
      y: center.y - plan.panOrigin.y,
    };
    const projectedPan = applyMatrix(plan.centerToPan, offset);
    const pan = {
      x: constrainedPanAxis(
        projectedPan.x,
        plan.centerToPan.xx,
        plan.centerToPan.xy,
        plan.currentPan.x,
        plan.panRange,
      ),
      y: constrainedPanAxis(
        projectedPan.y,
        plan.centerToPan.yx,
        plan.centerToPan.yy,
        plan.currentPan.y,
        plan.panRange,
      ),
    };
    const centerOffset = applyMatrix(plan.panToCenter, pan);

    return {
      pan,
      placement: {
        center: {
          x: plan.panOrigin.x + centerOffset.x,
          y: plan.panOrigin.y + centerOffset.y,
        },
        size: plan.current.size,
      },
    };
  }

  return {
    current: plan.current,
    panRange: plan.panRange,
    zoomRange: plan.zoomRange,
    zoom,
    constrain,
  };
}

function scalePlan(
  plan: PhotoPlacementPlan,
  unitScale: number,
): PhotoPlacementPlan {
  return {
    ...plan,
    current: scalePlacement(plan.current, unitScale),
    panOrigin: scaleVector(plan.panOrigin, unitScale),
    panToCenter: scaleMatrix(plan.panToCenter, unitScale),
    centerToPan: scaleMatrix(plan.centerToPan, 1 / unitScale),
    centerPerZoom: scaleVector(plan.centerPerZoom, unitScale),
    sizePerZoom: {
      width: plan.sizePerZoom.width * unitScale,
      height: plan.sizePerZoom.height * unitScale,
    },
  };
}

function scalePlacement(
  placement: PhotoPlacement,
  unitScale: number,
): PhotoPlacement {
  return {
    center: scaleVector(placement.center, unitScale),
    size: {
      width: placement.size.width * unitScale,
      height: placement.size.height * unitScale,
    },
  };
}

function scaleVector(vector: Vector2, unitScale: number): Vector2 {
  return {
    x: vector.x * unitScale,
    y: vector.y * unitScale,
  };
}

function scaleMatrix(matrix: Matrix2, scale: number): Matrix2 {
  return {
    xx: matrix.xx * scale,
    xy: matrix.xy * scale,
    yx: matrix.yx * scale,
    yy: matrix.yy * scale,
  };
}

function applyMatrix(matrix: Matrix2, vector: Vector2): Vector2 {
  return {
    x: matrix.xx * vector.x + matrix.xy * vector.y,
    y: matrix.yx * vector.x + matrix.yy * vector.y,
  };
}

function constrainedPanAxis(
  projected: number,
  matrixX: number,
  matrixY: number,
  fallback: number,
  range: NumberRange,
) {
  if (Math.abs(matrixX) + Math.abs(matrixY) <= Number.EPSILON) {
    return fallback;
  }
  return clampToRange(projected, range);
}

function clampToRange(value: number, range: NumberRange) {
  return Math.min(range.maximum, Math.max(range.minimum, value));
}
