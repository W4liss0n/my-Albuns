export const scrollIntoPointerViewportScript =
  "arguments[0].scrollIntoView({ block: 'center', inline: 'nearest' });";

export const measureVisiblePointerGeometryScript = `
  const visiblePoint = (element, label, xRatio = null) => {
    const bounds = element.getBoundingClientRect();
    const left = Math.max(0, bounds.left);
    const right = Math.min(window.innerWidth, bounds.right);
    const top = Math.max(0, bounds.top);
    const bottom = Math.min(window.innerHeight, bounds.bottom);
    if (right <= left || bottom <= top) {
      throw new Error(label + " is outside the pointer viewport");
    }
    const requestedX = xRatio === null
      ? (left + right) / 2
      : bounds.left + bounds.width * xRatio;
    if (requestedX < left || requestedX > right) {
      throw new Error(label + " sourceXRatio is outside the pointer viewport");
    }
    return {
      x: Math.round(requestedX),
      y: Math.round((top + bottom) / 2),
    };
  };
  const sourceXRatio = typeof arguments[3] === "number"
    ? arguments[3]
    : null;
  return {
    source: visiblePoint(arguments[0], "drag source", sourceXRatio),
    target: visiblePoint(arguments[1], "drag target"),
    dropTarget: arguments[2]
      ? visiblePoint(arguments[2], "drop target")
      : null,
  };
`;

function pointerThresholdPoint(source, target) {
  const deltaX = target.x - source.x;
  const deltaY = target.y - source.y;
  const distance = Math.hypot(deltaX, deltaY);
  if (distance === 0) return { x: source.x + 10, y: source.y };
  return {
    x: Math.round(source.x + (deltaX / distance) * 10),
    y: Math.round(source.y + (deltaY / distance) * 10),
  };
}

export function buildCapturedPointerGestureActions({
  dropTarget = null,
  phase,
  source,
  target,
}) {
  const threshold = pointerThresholdPoint(source, target);
  const actions = [
    {
      type: "pointerMove",
      duration: 0,
      origin: "viewport",
      x: source.x,
      y: source.y,
    },
    { type: "pointerDown", button: 0 },
    { type: "pause", duration: 80 },
    {
      type: "pointerMove",
      duration: 120,
      origin: "viewport",
      x: threshold.x,
      y: threshold.y,
    },
    {
      type: "pointerMove",
      duration: 450,
      origin: "viewport",
      x: target.x,
      y: target.y,
    },
    { type: "pause", duration: 100 },
  ];

  if (phase !== "drop") return actions;
  if (dropTarget) {
    actions.push(
      {
        type: "pointerMove",
        duration: 450,
        origin: "viewport",
        x: dropTarget.x,
        y: dropTarget.y,
      },
      { type: "pause", duration: 100 },
    );
  }
  actions.push({ type: "pointerUp", button: 0 });
  return actions;
}
