// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readFileSync } from "node:fs";

import { describe, expect, test } from "vitest";

const themeStyles = readFileSync("src/ui/theme.css", "utf8") as string;
const mediaPanelStyles = readFileSync(
  "src/components/MediaPanel.css",
  "utf8",
) as string;
const globalStyles = readFileSync(
  "src/global/GlobalShell.css",
  "utf8",
) as string;
const sharedStyles = readFileSync("src/ui/ui.css", "utf8") as string;
const newProjectStyles = readFileSync(
  "src/global/NewProjectFlow.css",
  "utf8",
) as string;

function themeColor(name: string): string {
  const match = themeStyles.match(
    new RegExp(`--${name}:\\s*(#[0-9a-f]{6});`, "i"),
  );
  expect(match, `missing literal color token --${name}`).not.toBeNull();
  return match![1];
}

function relativeLuminance(color: string): number {
  const channels = color
    .slice(1)
    .match(/.{2}/g)!
    .map((channel) => Number.parseInt(channel, 16) / 255)
    .map((channel) =>
      channel <= 0.04045
        ? channel / 12.92
        : ((channel + 0.055) / 1.055) ** 2.4,
    );
  return (
    0.2126 * channels[0] +
    0.7152 * channels[1] +
    0.0722 * channels[2]
  );
}

function contrastRatio(first: string, second: string): number {
  const firstLuminance = relativeLuminance(first);
  const secondLuminance = relativeLuminance(second);
  const lighter = Math.max(firstLuminance, secondLuminance);
  const darker = Math.min(firstLuminance, secondLuminance);
  return (lighter + 0.05) / (darker + 0.05);
}

describe("semantic color contracts", () => {
  test("keeps readable supporting text above 4.5:1 on every application surface", () => {
    const supportingText = themeColor("ui-text-muted");
    const surfaces = [
      "ui-canvas",
      "ui-surface",
      "ui-surface-raised",
      "ui-surface-muted",
      "ui-panel-surface",
    ];

    for (const surface of surfaces) {
      expect(
        contrastRatio(supportingText, themeColor(surface)),
        `--ui-text-muted on --${surface}`,
      ).toBeGreaterThanOrEqual(4.5);
    }
  });

  test("keeps normal text on filled accent controls above 4.5:1", () => {
    expect(
      contrastRatio(
        themeColor("ui-on-accent"),
        themeColor("ui-accent-fill"),
      ),
    ).toBeGreaterThanOrEqual(4.5);
  });

  test("keeps accent-colored labels distinct from spatial selection blue", () => {
    for (const surface of ["ui-surface", "ui-surface-raised"]) {
      expect(
        contrastRatio(themeColor("ui-accent-text"), themeColor(surface)),
        `--ui-accent-text on --${surface}`,
      ).toBeGreaterThanOrEqual(4.5);
    }
  });

  test("keeps range and scrollbar affordances above 3:1", () => {
    const nonTextPairs = [
      ["ui-range-track", "ui-surface-raised"],
      ["ui-range-thumb-border", "ui-surface-raised"],
      ["ui-scrollbar-thumb", "ui-canvas"],
      ["ui-scrollbar-thumb", "ui-panel-surface"],
      ["ui-scrollbar-thumb", "ui-border"],
    ] as const;

    for (const [foreground, background] of nonTextPairs) {
      expect(
        contrastRatio(themeColor(foreground), themeColor(background)),
        `--${foreground} on --${background}`,
      ).toBeGreaterThanOrEqual(3);
    }
  });

  test("keeps the Canvas scrollbar indicator lighter with a perceivable edge", () => {
    const track = themeColor("ui-canvas-scrollbar-track");
    const thumb = themeColor("ui-canvas-scrollbar-thumb");
    const thumbHover = themeColor("ui-canvas-scrollbar-thumb-hover");
    const thumbBorder = themeColor("ui-canvas-scrollbar-thumb-border");

    expect(track).toBe(themeColor("ui-border"));
    expect(relativeLuminance(thumb), "Canvas scrollbar thumb luminance")
      .toBeGreaterThan(relativeLuminance(track));
    expect(
      relativeLuminance(thumbHover),
      "Canvas scrollbar hover/focus thumb luminance",
    ).toBeGreaterThan(relativeLuminance(track));
    expect(
      contrastRatio(thumbBorder, track),
      "Canvas scrollbar thumb edge on its track",
    ).toBeGreaterThanOrEqual(3);
  });

  test("does not fade readable text inside active controls", () => {
    expect(mediaPanelStyles).toMatch(
      /\.media-folder-chip small\s*\{[^}]*opacity:\s*1;/s,
    );
    expect(globalStyles).toMatch(
      /\.global-action-stack kbd\s*\{[^}]*opacity:\s*1;/s,
    );
  });

  test("uses the accessible fill token wherever small white accent text appears", () => {
    expect(sharedStyles).toMatch(
      /\.ui-action-button--primary\s*\{[^}]*color:\s*var\(--ui-on-accent\);[^}]*background:\s*var\(--ui-accent-fill\);/s,
    );
    expect(mediaPanelStyles).toMatch(
      /\.media-folder-chip\.active\s*\{[^}]*color:\s*var\(--ui-on-accent\);[^}]*background:\s*var\(--ui-accent-fill\);/s,
    );
    expect(newProjectStyles).toMatch(
      /\.new-project-steps li\[aria-current="step"\] > span\s*\{[^}]*color:\s*var\(--ui-on-accent\);[^}]*background:\s*var\(--ui-accent-fill\);/s,
    );
  });
});
