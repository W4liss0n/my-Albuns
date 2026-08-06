import { expect, test, vi } from "vitest";
import { installDesktopWebViewPolicy } from "./desktopWebViewPolicy";

function createPolicyDocument() {
  const policyDocument = document.implementation.createHTMLDocument();
  const uninstall = installDesktopWebViewPolicy(policyDocument);
  return { policyDocument, uninstall };
}

test("leaves native browser accelerators to the host policy", () => {
  const { policyDocument, uninstall } = createPolicyDocument();
  const reload = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    ctrlKey: true,
    key: "r",
  });
  const ctrlWheel = new WheelEvent("wheel", {
    bubbles: true,
    cancelable: true,
    ctrlKey: true,
  });
  const contextMenu = new MouseEvent("contextmenu", {
    bubbles: true,
    cancelable: true,
  });

  policyDocument.body.dispatchEvent(reload);
  policyDocument.body.dispatchEvent(ctrlWheel);
  policyDocument.body.dispatchEvent(contextMenu);

  expect(reload.defaultPrevented).toBe(false);
  expect(ctrlWheel.defaultPrevented).toBe(false);
  expect(contextMenu.defaultPrevented).toBe(false);
  uninstall();
});

test("prevents drop navigation while preserving a product drop handler", () => {
  const { policyDocument, uninstall } = createPolicyDocument();
  const productHandler = vi.fn();
  policyDocument.body.addEventListener("drop", productHandler);
  const dragOver = new Event("dragover", {
    bubbles: true,
    cancelable: true,
  });
  const drop = new Event("drop", {
    bubbles: true,
    cancelable: true,
  });

  policyDocument.body.dispatchEvent(dragOver);
  policyDocument.body.dispatchEvent(drop);

  expect(dragOver.defaultPrevented).toBe(true);
  expect(drop.defaultPrevented).toBe(true);
  expect(productHandler).toHaveBeenCalledOnce();
  uninstall();
});

test("prevents native link navigation", () => {
  const { policyDocument, uninstall } = createPolicyDocument();
  const link = policyDocument.createElement("a");
  link.href = "https://example.com";
  policyDocument.body.append(link);
  const event = new MouseEvent("click", {
    bubbles: true,
    cancelable: true,
    button: 0,
  });

  link.dispatchEvent(event);

  expect(event.defaultPrevented).toBe(true);
  uninstall();
});

test.each([1, 3, 4])(
  "prevents the native action for auxiliary mouse button %i",
  (button) => {
    const { policyDocument, uninstall } = createPolicyDocument();
    const mouseDown = new MouseEvent("mousedown", {
      bubbles: true,
      cancelable: true,
      button,
    });
    const auxiliaryClick = new MouseEvent("auxclick", {
      bubbles: true,
      cancelable: true,
      button,
    });

    policyDocument.body.dispatchEvent(mouseDown);
    policyDocument.body.dispatchEvent(auxiliaryClick);

    expect(mouseDown.defaultPrevented).toBe(true);
    expect(auxiliaryClick.defaultPrevented).toBe(true);
    uninstall();
  },
);

test("uninstalls every DOM browser-default suppression", () => {
  const { policyDocument, uninstall } = createPolicyDocument();
  const link = policyDocument.createElement("a");
  link.href = "https://example.com";
  policyDocument.body.append(link);
  uninstall();
  const click = new MouseEvent("click", {
    bubbles: true,
    cancelable: true,
    button: 0,
  });

  link.dispatchEvent(click);

  expect(click.defaultPrevented).toBe(false);
});
