import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

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
