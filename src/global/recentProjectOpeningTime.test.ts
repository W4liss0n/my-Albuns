import { expect, test } from "vitest";

import { recentProjectOpeningTime } from "./recentProjectOpeningTime";

const now = new Date(2026, 8, 22, 16, 0);

test("formats the opening time using local calendar days and pt-BR time", () => {
  const today = new Date(2026, 8, 22, 14, 30);
  const yesterday = new Date(2026, 8, 21, 9, 15);
  const older = new Date(2026, 8, 18, 9, 15);
  expect(recentProjectOpeningTime(today.getTime(), now)).toEqual({
    label: "Hoje", fullLabel: "Hoje às 14:30", dateTime: today.toISOString(),
  });
  expect(recentProjectOpeningTime(yesterday.getTime(), now)).toMatchObject({
    label: "Ontem", fullLabel: "Ontem às 09:15",
  });
  expect(recentProjectOpeningTime(older.getTime(), now)).toMatchObject({
    label: "18/09/2026", fullLabel: "18/09/2026 às 09:15",
  });
});

test("uses calendar boundaries, including across the year, rather than elapsed hours", () => {
  const justAfterMidnight = new Date(2027, 0, 1, 0, 1);
  const previousDay = new Date(2026, 11, 31, 23, 59);
  expect(recentProjectOpeningTime(previousDay.getTime(), justAfterMidnight)?.fullLabel)
    .toBe("Ontem às 23:59");
});

test("omits missing or malformed timestamps", () => {
  for (const value of [null, -1, 1.5, Number.MAX_SAFE_INTEGER + 1, NaN]) {
    expect(recentProjectOpeningTime(value, now)).toBeNull();
  }
});
