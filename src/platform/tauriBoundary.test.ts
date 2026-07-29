import { expect, test } from "vitest";

const sourceFiles = import.meta.glob("../**/*.{ts,tsx}", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

function findOffenders(
  isOffender: (path: string, source: string) => boolean,
) {
  return Object.entries(sourceFiles)
    .filter(([path, source]) => isOffender(path, source))
    .map(([path]) => path)
    .sort();
}

test("keeps Tauri dependencies inside platform adapters", () => {
  const tauriPackagePrefix = ["@tauri", "-apps"].join("");
  const offenders = findOffenders(
    (path, source) =>
      source.includes(tauriPackagePrefix) &&
      !path.startsWith("./") &&
      !path.startsWith("../platform/"),
  );

  expect(offenders).toEqual([]);
});

test("selects concrete platform adapters only at the composition root", () => {
  const adapterPaths = [
    ["platform", "tauriProjectBridge"].join("/"),
    ["platform", "graphics"].join("/"),
    ["platform", "tauriLogger"].join("/"),
  ];
  const offenders = findOffenders(
    (path, source) =>
      adapterPaths.some((adapterPath) => source.includes(adapterPath)) &&
      path !== "../main.tsx",
  );

  expect(offenders).toEqual([]);
});
