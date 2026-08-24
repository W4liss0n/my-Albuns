import { expect, test } from "vitest";

import {
  displayUnitLabel,
  formatPhysicalMeasurement,
  parsePhysicalText,
} from "./physicalMeasurements";

test("formats the canonical inch label as pol", () => {
  expect(displayUnitLabel("in")).toBe("pol");
  expect(formatPhysicalMeasurement(25_400, "in")).toBe("1 pol");
});

test("parses only decimal measurements that map to whole micrometers", () => {
  expect(parsePhysicalText("12,5", "mm")).toBe(12_500);
  expect(parsePhysicalText("1", "in")).toBe(25_400);
  expect(parsePhysicalText("0.0001", "mm")).toBeNull();
});

test("rejects oversized measurement text before attempting arbitrary precision work", () => {
  expect(parsePhysicalText(`1.${"0".repeat(256)}`, "mm")).toBeNull();
  expect(parsePhysicalText("9".repeat(256), "mm")).toBeNull();
});
