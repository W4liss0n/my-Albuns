import { afterEach } from "vitest";

// Test files that opt into the node environment have no DOM to prepare.
if (typeof document !== "undefined") {
  await import("@testing-library/jest-dom/vitest");
  const { cleanup } = await import("@testing-library/react");

  afterEach(cleanup);

  // jsdom has no layout; gesture tests supply their own hit targets when needed.
  Object.defineProperty(document, "elementFromPoint", {
    configurable: true,
    value: () => null,
  });

  Object.defineProperty(HTMLCanvasElement.prototype, "getContext", {
    configurable: true,
    value: () => null,
  });
}
