import { webdriverElementKey } from "./UiAcceptance.mjs";

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

function elementReference(elementId) {
  return { [webdriverElementKey]: elementId };
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

  if (action.type === "drag") {
    const targetId = await locateSelector(action.targetSelector);
    const dropTargetId = action.dropTargetSelector
      ? await locateSelector(action.dropTargetSelector)
      : null;
    const pointerActions = [
      {
        type: "pointerMove",
        duration: 0,
        origin: elementReference(elementId),
        x: 0,
        y: 0,
      },
      { type: "pointerDown", button: 0 },
      {
        type: "pointerMove",
        duration: 220,
        origin: elementReference(targetId),
        x: 0,
        y: 0,
      },
    ];
    if (action.phase === "drop") {
      if (dropTargetId) {
        pointerActions.push({
          type: "pointerMove",
          duration: 220,
          origin: elementReference(dropTargetId),
          x: 0,
          y: 0,
        });
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
