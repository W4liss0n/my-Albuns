import { expect, test } from "vitest";

import {
  createPhysicalFieldDraft,
  displayUnitLabel,
  editPhysicalFieldDraft,
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

test("restores the exact micrometers behind a rounded inch presentation", () => {
  const generated = createPhysicalFieldDraft(400_000, "in");
  expect(generated.text).toBe("15.748");

  const shortened = editPhysicalFieldDraft(generated, "15.74", "in");
  expect(shortened).toMatchObject({
    hasExactValue: true,
    valueUm: 399_796,
  });

  expect(editPhysicalFieldDraft(shortened, "15.748", "in")).toMatchObject({
    hasExactValue: true,
    text: "15.748",
    valueUm: 400_000,
  });
});

test("rejects oversized measurement text before attempting arbitrary precision work", () => {
  expect(parsePhysicalText(`1.${"0".repeat(256)}`, "mm")).toBeNull();
  expect(parsePhysicalText("9".repeat(256), "mm")).toBeNull();
});
