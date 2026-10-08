import type {
  Matrix2,
  NormalizedPan,
  NumberRange,
  PhotoPlacement,
  PhotoPlacementPlan,
  VectorUm,
} from "../../domain/project";

export interface CanvasPoint {
  x: number;
  y: number;
}

export interface CanvasSize {
  width: number;
  height: number;
}

export interface CanvasPhotoPlacement {
  center: CanvasPoint;
  size: CanvasSize;
}

interface CanvasPanTransform {
  xx: number;
  xy: number;
  yx: number;
  yy: number;
}

interface CanvasPhotoPlacementPlan {
  currentPan: NormalizedPan;
  currentZoom: number;
  panRange: NumberRange;
  zoomRange: NumberRange;
  current: CanvasPhotoPlacement;
  panOrigin: CanvasPoint;
  panToCenter: CanvasPanTransform;
  panToCenterPerZoom: CanvasPanTransform;
  sizePerZoom: CanvasSize;
}

export interface ConstrainedPhotoPlacement {
  pan: NormalizedPan;
  zoom: number;
  placement: CanvasPhotoPlacement;
}

/** A previewed Photo center and the Zoom it is drawn at. */
export interface PhotoZoomStart {
  center: CanvasPoint;
  zoom: number;
}

export interface PhotoGeometry {
  current: CanvasPhotoPlacement;
  panRange: NumberRange;
  zoomRange: NumberRange;
  /**
   * Zooms keeping fixed the Photo point at the Frame center, as the Core
   * does; Preenchimento clamps the Pan when zooming out near an edge.
   * Starts from the committed placement unless a preview is given.
   */
  zoom(targetZoom: number, from?: PhotoZoomStart): ConstrainedPhotoPlacement;
  constrain(
    center: CanvasPoint,
    targetZoom?: number,
  ): ConstrainedPhotoPlacement;
}

const CENTERED_PAN: NormalizedPan = { x: 0, y: 0 };

export function createPhotoGeometry(
  sourcePlan: PhotoPlacementPlan,
  unitScale = 1,
): PhotoGeometry {
  const plan = scalePlan(sourcePlan, unitScale);

  function zoom(
    targetZoom: number,
    from: PhotoZoomStart = {
      center: plan.current.center,
      zoom: plan.currentZoom,
    },
  ): ConstrainedPhotoPlacement {
    const boundedZoom = clampToRange(targetZoom, plan.zoomRange);
    const ratio = boundedZoom / from.zoom;
    // Projecting the scaled offset onto the Pan axes equals the Core's
    // per-axis rule; an axis without room keeps the Photo centered on it.
    return constrainAt(
      {
        x: plan.panOrigin.x + (from.center.x - plan.panOrigin.x) * ratio,
        y: plan.panOrigin.y + (from.center.y - plan.panOrigin.y) * ratio,
      },
      boundedZoom,
      CENTERED_PAN,
    );
  }

  function constrain(
    center: CanvasPoint,
    targetZoom = plan.currentZoom,
  ): ConstrainedPhotoPlacement {
    return constrainAt(
      center,
      clampToRange(targetZoom, plan.zoomRange),
      plan.currentPan,
    );
  }

  function constrainAt(
    center: CanvasPoint,
    boundedZoom: number,
    fallbackPan: NormalizedPan,
  ): ConstrainedPhotoPlacement {
    const panToCenter = panToCenterAtZoom(boundedZoom);
    const offset = {
      x: center.x - plan.panOrigin.x,
      y: center.y - plan.panOrigin.y,
    };
    const pan = {
      x: projectPanAxis(
        offset,
        panToCenter.xx,
        panToCenter.yx,
        fallbackPan.x,
        plan.panRange,
      ),
      y: projectPanAxis(
        offset,
        panToCenter.xy,
        panToCenter.yy,
        fallbackPan.y,
        plan.panRange,
      ),
    };

    return {
      pan,
      zoom: boundedZoom,
      placement: placementAt(pan, boundedZoom),
    };
  }

  function placementAt(
    pan: NormalizedPan,
    boundedZoom: number,
  ): CanvasPhotoPlacement {
    const delta = boundedZoom - plan.currentZoom;
    const centerOffset = applyMatrix(
      panToCenterAtZoom(boundedZoom),
      pan,
    );

    return {
      center: {
        x: plan.panOrigin.x + centerOffset.x,
        y: plan.panOrigin.y + centerOffset.y,
      },
      size: {
        width:
          plan.current.size.width + plan.sizePerZoom.width * delta,
        height:
          plan.current.size.height + plan.sizePerZoom.height * delta,
      },
    };
  }

  function panToCenterAtZoom(
    targetZoom: number,
  ): CanvasPanTransform {
    const delta = targetZoom - plan.currentZoom;
    return {
      xx:
        plan.panToCenter.xx +
        plan.panToCenterPerZoom.xx * delta,
      xy:
        plan.panToCenter.xy +
        plan.panToCenterPerZoom.xy * delta,
      yx:
        plan.panToCenter.yx +
        plan.panToCenterPerZoom.yx * delta,
      yy:
        plan.panToCenter.yy +
        plan.panToCenterPerZoom.yy * delta,
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
): CanvasPhotoPlacementPlan {
  return {
    ...plan,
    current: scalePlacement(plan.current, unitScale),
    panOrigin: scaleVector(plan.panOrigin, unitScale),
    panToCenter: scaleMatrix(plan.panToCenter, unitScale),
    panToCenterPerZoom: scaleMatrix(
      plan.panToCenterPerZoom,
      unitScale,
    ),
    sizePerZoom: {
      width: plan.sizePerZoom.width * unitScale,
      height: plan.sizePerZoom.height * unitScale,
    },
  };
}

function scalePlacement(
  placement: PhotoPlacement,
  unitScale: number,
): CanvasPhotoPlacement {
  return {
    center: scaleVector(placement.center, unitScale),
    size: {
      width: placement.size.width * unitScale,
      height: placement.size.height * unitScale,
    },
  };
}

function scaleVector(
  vector: VectorUm,
  unitScale: number,
): CanvasPoint {
  return {
    x: vector.x * unitScale,
    y: vector.y * unitScale,
  };
}

function scaleMatrix(
  matrix: Matrix2,
  scale: number,
): CanvasPanTransform {
  return {
    xx: matrix.xx * scale,
    xy: matrix.xy * scale,
    yx: matrix.yx * scale,
    yy: matrix.yy * scale,
  };
}

function applyMatrix(
  matrix: CanvasPanTransform,
  vector: NormalizedPan,
): CanvasPoint {
  return {
    x: matrix.xx * vector.x + matrix.xy * vector.y,
    y: matrix.yx * vector.x + matrix.yy * vector.y,
  };
}

function projectPanAxis(
  offset: CanvasPoint,
  axisX: number,
  axisY: number,
  fallback: number,
  range: NumberRange,
) {
  const squaredLength = axisX * axisX + axisY * axisY;
  if (squaredLength <= Number.EPSILON) {
    return fallback;
  }
  return clampToRange(
    (offset.x * axisX + offset.y * axisY) / squaredLength,
    range,
  );
}

function clampToRange(value: number, range: NumberRange) {
  return Math.min(range.maximum, Math.max(range.minimum, value));
}
