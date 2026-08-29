import {
  webdriverElementId,
  webdriverElementKey,
} from "./UiAcceptance.mjs";

const webdriverKeys = Object.freeze({
  ArrowDown: "\uE015",
  ArrowLeft: "\uE012",
  ArrowRight: "\uE014",
  ArrowUp: "\uE013",
  Enter: "\uE007",
  Escape: "\uE00C",
  Space: "\uE00D",
  Tab: "\uE004",
  Digit0: "0",
  Minus: "\uE027",
  Plus: "\uE025",
});

const webdriverModifiers = Object.freeze({
  Control: "\uE009",
});

const cdpArrowKeys = Object.freeze({
  ArrowLeft: {
    code: "ArrowLeft",
    key: "ArrowLeft",
    virtualKeyCode: 37,
  },
  ArrowRight: {
    code: "ArrowRight",
    key: "ArrowRight",
    virtualKeyCode: 39,
  },
});

function elementReference(elementId) {
  return { [webdriverElementKey]: elementId };
}

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

const activeSheetReorderSelector = [
  '[data-reorder-surface][data-reorder-state="preview"]',
  '[data-reorder-surface][data-reorder-state="invalid"]',
].join(",");

async function cancelPointerGestureWithEscape({
  execute,
  locateSelector,
  request,
  sessionId,
}) {
  let cancellationError;
  try {
    const bodyId = await locateSelector("body");
    try {
      await request(
        "POST",
        `/session/${sessionId}/element/${encodeURIComponent(bodyId)}/value`,
        { text: webdriverKeys.Escape, value: [webdriverKeys.Escape] },
      );
    } catch {
      // A public-state check below decides whether the deterministic fallback is needed.
    }

    let reorderRemainsActive = await execute(
      "return Boolean(document.querySelector(arguments[0]));",
      [activeSheetReorderSelector],
    );
    if (reorderRemainsActive) {
      await execute(`
        const target = document.activeElement ?? document.body;
        target.dispatchEvent(new KeyboardEvent("keydown", {
          key: "Escape",
          bubbles: true,
          cancelable: true,
        }));
        target.dispatchEvent(new KeyboardEvent("keyup", {
          key: "Escape",
          bubbles: true,
          cancelable: true,
        }));
      `);
      reorderRemainsActive = await execute(
        "return Boolean(document.querySelector(arguments[0]));",
        [activeSheetReorderSelector],
      );
    }
    if (reorderRemainsActive) {
      throw new Error("Escape did not cancel the active pointer gesture");
    }
  } catch (error) {
    cancellationError = error;
  }

  try {
    await request("DELETE", `/session/${sessionId}/actions`);
  } catch (error) {
    cancellationError ??= error;
  }
  if (cancellationError) throw cancellationError;
}

export function requiresBrowserZoomInvariant(actions) {
  return actions.some(
    (action) =>
      action.type === "wheel" && action.modifiers?.includes("Control"),
  );
}

export async function captureBrowserZoomState({ execute }) {
  return execute(`
    return {
      devicePixelRatio: window.devicePixelRatio,
      innerHeight: window.innerHeight,
      innerWidth: window.innerWidth,
      visualViewportScale: window.visualViewport?.scale ?? null,
    };
  `);
}

export function assertBrowserZoomUnchanged({ after, before, label }) {
  const measuredProperties = [
    "devicePixelRatio",
    "innerHeight",
    "innerWidth",
    "visualViewportScale",
  ];
  const changes = measuredProperties
    .filter((property) => !Object.is(before[property], after[property]))
    .map((property) => `${property}: ${before[property]} -> ${after[property]}`);
  if (changes.length > 0) {
    throw new Error(`${label} changed browser zoom (${changes.join(", ")})`);
  }
}

export async function neutralizeUiAcceptancePointer({
  request,
  sessionId,
  viewport,
}) {
  await request("DELETE", `/session/${sessionId}/actions`);
  await request("POST", `/session/${sessionId}/actions`, {
    actions: [
      {
        type: "pointer",
        id: "acceptance-pointer",
        parameters: { pointerType: "mouse" },
        actions: [
          {
            type: "pointerMove",
            duration: 0,
            origin: "viewport",
            x: 2,
            y: Math.max(2, viewport.height - 2),
          },
        ],
      },
    ],
  });
}

export async function performUiAcceptanceAction({
  action,
  execute,
  locateSelector,
  locateText,
  request,
  sessionId,
}) {
  if (action.type === "key") {
    const value = webdriverKeys[action.key];
    if (!value) throw new Error(`Unsupported UI acceptance key: ${action.key}`);
    const modifierValues = (action.modifiers ?? []).map(
      (modifier) => webdriverModifiers[modifier],
    );
    const cdpArrow = cdpArrowKeys[action.key];
    if (modifierValues.length === 0 && cdpArrow) {
      for (const type of ["rawKeyDown", "keyUp"]) {
        await request(
          "POST",
          `/session/${sessionId}/ms/cdp/execute`,
          {
            cmd: "Input.dispatchKeyEvent",
            params: {
              code: cdpArrow.code,
              key: cdpArrow.key,
              nativeVirtualKeyCode: cdpArrow.virtualKeyCode,
              type,
              windowsVirtualKeyCode: cdpArrow.virtualKeyCode,
            },
          },
        );
      }
      return;
    }
    if (modifierValues.length === 0) {
      const activeElement = await execute("return document.activeElement;");
      const activeElementId = webdriverElementId(activeElement);
      await request(
        "POST",
        `/session/${sessionId}/element/${encodeURIComponent(activeElementId)}/value`,
        { text: value, value: [value] },
      );
      return;
    }
    await request("POST", `/session/${sessionId}/actions`, {
      actions: [
        {
          type: "key",
          id: "acceptance-keyboard",
          actions: [
            ...modifierValues.map((modifier) => ({
              type: "keyDown",
              value: modifier,
            })),
            { type: "keyDown", value },
            { type: "keyUp", value },
            ...[...modifierValues].reverse().map((modifier) => ({
              type: "keyUp",
              value: modifier,
            })),
          ],
        },
      ],
    });
    return;
  }

  const elementId =
    action.type === "click-text"
      ? await locateText(action.text)
      : await locateSelector(action.selector);
  const encodedElementId = encodeURIComponent(elementId);

  if (action.type === "assert") return;

  if (action.type === "context-click" || action.type === "pointer-click") {
    const button = action.type === "context-click" ? 2 : 0;
    await request("POST", `/session/${sessionId}/actions`, {
      actions: [
        {
          type: "pointer",
          id: "acceptance-pointer",
          parameters: { pointerType: "mouse" },
          actions: [
            {
              type: "pointerMove",
              duration: 0,
              origin: elementReference(elementId),
              x: 0,
              y: 0,
            },
            { type: "pointerDown", button },
            { type: "pointerUp", button },
          ],
        },
      ],
    });
    return;
  }

  if (action.type === "click" || action.type === "click-text") {
    if (action.modifiers?.includes("Control")) {
      await request("POST", `/session/${sessionId}/actions`, {
        actions: [
          {
            type: "key",
            id: "acceptance-keyboard",
            actions: [
              { type: "keyDown", value: webdriverModifiers.Control },
              { type: "pause", duration: 0 },
              { type: "pause", duration: 0 },
              { type: "pause", duration: 0 },
              { type: "keyUp", value: webdriverModifiers.Control },
            ],
          },
          {
            type: "pointer",
            id: "acceptance-pointer",
            parameters: { pointerType: "mouse" },
            actions: [
              { type: "pause", duration: 0 },
              {
                type: "pointerMove",
                duration: 0,
                origin: elementReference(elementId),
                x: 0,
                y: 0,
              },
              { type: "pointerDown", button: 0 },
              { type: "pointerUp", button: 0 },
              { type: "pause", duration: 0 },
            ],
          },
        ],
      });
      return;
    }
    await request(
      "POST",
      `/session/${sessionId}/element/${encodedElementId}/click`,
      {},
    );
    return;
  }

  if (action.type === "focus") {
    const focused = await execute(
      "arguments[0].focus({ preventScroll: false }); return document.activeElement === arguments[0];",
      [elementReference(elementId)],
    );
    if (!focused) throw new Error(`Element did not receive focus: ${action.selector}`);
    return;
  }

  if (action.type === "hover") {
    await request("POST", `/session/${sessionId}/actions`, {
      actions: [
        {
          type: "pointer",
          id: "acceptance-pointer",
          parameters: { pointerType: "mouse" },
          actions: [
            {
              type: "pointerMove",
              duration: 0,
              origin: elementReference(elementId),
              x: 0,
              y: 0,
            },
          ],
        },
      ],
    });
    return;
  }

  if (action.type === "input") {
    await request(
      "POST",
      `/session/${sessionId}/element/${encodedElementId}/clear`,
      {},
    );
    await request(
      "POST",
      `/session/${sessionId}/element/${encodedElementId}/value`,
      { text: action.value },
    );
    return;
  }

  if (action.type === "wheel") {
    const modifierActions = action.modifiers?.includes("Control")
      ? [
          {
            type: "key",
            id: "acceptance-keyboard",
            actions: [
              { type: "keyDown", value: webdriverModifiers.Control },
              { type: "pause", duration: 0 },
              { type: "keyUp", value: webdriverModifiers.Control },
            ],
          },
        ]
      : [];
    await request("POST", `/session/${sessionId}/actions`, {
      actions: [
        ...modifierActions,
        {
          type: "wheel",
          id: "acceptance-wheel",
          actions: [
            { type: "pause", duration: 0 },
            {
              type: "scroll",
              duration: 0,
              origin: elementReference(elementId),
              x: 0,
              y: 0,
              deltaX: action.deltaX ?? 0,
              deltaY: action.deltaY,
            },
            { type: "pause", duration: 0 },
          ],
        },
      ],
    });
    return;
  }

  if (action.type === "selection-drag") {
    const element = elementReference(elementId);
    const dragOffsets = await execute(
      `
        window.getSelection()?.removeAllRanges();
        const element = arguments[0];
        element.focus?.({ preventScroll: true });
        const rect = element.getBoundingClientRect();
        const horizontalInset = Math.min(8, Math.max(2, rect.width / 5));
        const verticalInset = Math.min(6, Math.max(2, rect.height / 4));
        const textControl =
          element instanceof HTMLInputElement ||
          element instanceof HTMLTextAreaElement;
        return {
          startX: Math.round(-rect.width / 2 + horizontalInset),
          startY: textControl
            ? 0
            : Math.round(-rect.height / 2 + verticalInset),
          endX: Math.round(rect.width / 2 - horizontalInset),
          endY: textControl
            ? 0
            : Math.round(rect.height / 2 - verticalInset),
        };
      `,
      [element],
    );
    await request("POST", `/session/${sessionId}/actions`, {
      actions: [
        {
          type: "pointer",
          id: "acceptance-selection-pointer",
          parameters: { pointerType: "mouse" },
          actions: [
            {
              type: "pointerMove",
              duration: 0,
              origin: element,
              x: dragOffsets.startX,
              y: dragOffsets.startY,
            },
            { type: "pointerDown", button: 0 },
            {
              type: "pointerMove",
              duration: 300,
              origin: element,
              x: dragOffsets.endX,
              y: dragOffsets.endY,
            },
            { type: "pointerUp", button: 0 },
          ],
        },
      ],
    });
    const observation = await execute(
      `
        const element = arguments[0];
        return {
          controlSelection:
            typeof element.selectionStart === "number" &&
            typeof element.selectionEnd === "number"
              ? element.selectionEnd - element.selectionStart
              : 0,
          documentSelection: window.getSelection()?.toString().length ?? 0,
        };
      `,
      [element],
    );
    const satisfied =
      action.expect === "control"
        ? observation.controlSelection > 0
        : action.expect === "text"
          ? observation.documentSelection > 0
          : observation.controlSelection === 0 &&
            observation.documentSelection === 0;
    if (!satisfied) {
      throw new Error(
        `Selection policy mismatch for ${action.selector}: expected=${action.expect}, observed=${JSON.stringify(observation)}`,
      );
    }
    return;
  }

  if (action.type === "drag") {
    const targetId = await locateSelector(action.targetSelector);
    const dropTargetId = action.dropTargetSelector
      ? await locateSelector(action.dropTargetSelector)
      : null;
    const sourceElement = elementReference(elementId);
    const targetElement = elementReference(targetId);
    const capturedPointerGesture = action.gesture === "pointer";
    if (capturedPointerGesture) {
      await execute(
        "arguments[0].scrollIntoView({ block: 'center', inline: 'nearest' });",
        [sourceElement],
      );
      if (dropTargetId) {
        await execute(
          "arguments[0].scrollIntoView({ block: 'center', inline: 'nearest' });",
          [elementReference(dropTargetId)],
        );
      }
    }
    const gestureGeometry = capturedPointerGesture
      ? await execute(
          `
            const visibleCenter = (element, label) => {
              const bounds = element.getBoundingClientRect();
              const left = Math.max(0, bounds.left);
              const right = Math.min(window.innerWidth, bounds.right);
              const top = Math.max(0, bounds.top);
              const bottom = Math.min(window.innerHeight, bounds.bottom);
              if (right <= left || bottom <= top) {
                throw new Error(label + " is outside the pointer viewport");
              }
              return {
                x: Math.round((left + right) / 2),
                y: Math.round((top + bottom) / 2),
              };
            };
            return {
              source: visibleCenter(arguments[0], "drag source"),
              target: visibleCenter(arguments[1], "drag target"),
              dropTarget: arguments[2]
                ? visibleCenter(arguments[2], "drop target")
                : null,
            };
          `,
          [
            sourceElement,
            targetElement,
            dropTargetId ? elementReference(dropTargetId) : null,
          ],
        )
      : null;
    const thresholdPoint = gestureGeometry
      ? pointerThresholdPoint(gestureGeometry.source, gestureGeometry.target)
      : null;
    const pointerActions = capturedPointerGesture
      ? [
          {
            type: "pointerMove",
            duration: 0,
            origin: "viewport",
            x: gestureGeometry.source.x,
            y: gestureGeometry.source.y,
          },
          { type: "pointerDown", button: 0 },
          { type: "pause", duration: 80 },
          {
            type: "pointerMove",
            duration: 120,
            origin: "viewport",
            x: thresholdPoint.x,
            y: thresholdPoint.y,
          },
          {
            type: "pointerMove",
            duration: 450,
            origin: "viewport",
            x: gestureGeometry.target.x,
            y: gestureGeometry.target.y,
          },
          { type: "pause", duration: 100 },
        ]
      : [
          {
            type: "pointerMove",
            duration: 0,
            origin: sourceElement,
            x: 0,
            y: 0,
          },
          { type: "pointerDown", button: 0 },
          {
            type: "pointerMove",
            duration: 220,
            origin: targetElement,
            x: 0,
            y: 0,
          },
        ];
    if (action.phase === "drop") {
      if (dropTargetId) {
        pointerActions.push({
          type: "pointerMove",
          duration: capturedPointerGesture ? 450 : 220,
          origin: capturedPointerGesture ? "viewport" : elementReference(dropTargetId),
          x: capturedPointerGesture ? gestureGeometry.dropTarget.x : 0,
          y: capturedPointerGesture ? gestureGeometry.dropTarget.y : 0,
        });
        if (capturedPointerGesture) {
          pointerActions.push({ type: "pause", duration: 100 });
        }
      }
      pointerActions.push({ type: "pointerUp", button: 0 });
    }
    await request("POST", `/session/${sessionId}/actions`, {
      actions: [
        {
          type: "pointer",
          id: "acceptance-pointer",
          parameters: { pointerType: "mouse" },
          actions: pointerActions,
        },
      ],
    });
    if (action.phase === "escape") {
      if (!capturedPointerGesture) {
        throw new Error("Escape termination requires a captured pointer gesture");
      }
      await cancelPointerGestureWithEscape({
        execute,
        locateSelector,
        request,
        sessionId,
      });
    }
    return;
  }

  throw new Error(`Unsupported UI acceptance action: ${action.type}`);
}

export async function captureUiAcceptanceScreenshot({
  captureSelector,
  locateSelector,
  request,
  sessionId,
}) {
  if (!captureSelector) {
    return request("GET", `/session/${sessionId}/screenshot`);
  }
  const elementId = await locateSelector(captureSelector);
  return request(
    "GET",
    `/session/${sessionId}/element/${encodeURIComponent(elementId)}/screenshot`,
  );
}
