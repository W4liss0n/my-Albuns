import { expect, test } from "vitest";
import projectWindowCapability from "../../src-tauri/capabilities/default.json?raw";
import projectWindowPermission from "../../src-tauri/permissions/project-window.json?raw";

const sourceFiles = import.meta.glob("../**/*.{ts,tsx}", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

const tauriCommandSources = {
  shared: ["./tauriLogger.ts"],
  project: ["./tauriProjectPorts.ts"],
} as const;

function findOffenders(
  isOffender: (path: string, source: string) => boolean,
) {
  return Object.entries(sourceFiles)
    .filter(([path, source]) => isOffender(path, source))
    .map(([path]) => path)
    .sort();
}

function extractInvokedCommands(sourcePaths: readonly string[]) {
  return new Set(
    sourcePaths.flatMap((path) =>
      Array.from(
        sourceFiles[path].matchAll(
          /\binvoke(?:<[^>]+>)?\(\s*["']([^"']+)["']/g,
        ),
        (match) => match[1],
      ),
    ),
  );
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
  const platformImport =
    /(?:from\s+|import\s*)["'][^"']*\/platform\//;
  const offenders = findOffenders(
    (path, source) =>
      path !== "../main.tsx" &&
      !path.startsWith("../platform/") &&
      platformImport.test(source),
  );

  expect(offenders).toEqual([]);
});

test("assigns every Tauri command adapter to an explicit surface", () => {
  const invokingSources = Object.entries(sourceFiles)
    .filter(
      ([path, source]) =>
        !path.includes(".test.") &&
        source.includes("@tauri-apps/api/core") &&
        /\binvoke(?:<[^>]+>)?\(/.test(source),
    )
    .map(([path]) => path)
    .sort();
  const assignedSources = [
    ...tauriCommandSources.shared,
    ...tauriCommandSources.project,
  ];

  expect(new Set(assignedSources).size).toBe(assignedSources.length);
  expect([...assignedSources].sort()).toEqual(invokingSources);
});

test("keeps the project-window capability aligned with the invoked commands", () => {
  const invokedCommands = extractInvokedCommands(
    [...tauriCommandSources.shared, ...tauriCommandSources.project],
  );
  const capability = JSON.parse(projectWindowCapability) as {
    permissions: string[];
  };
  const permissionManifest = JSON.parse(projectWindowPermission) as {
    permission: Array<{
      identifier: string;
      commands: { allow: string[]; deny: string[] };
    }>;
  };
  const permission = permissionManifest.permission[0];
  const allowedCommands = new Set(permission.commands.allow);

  expect(permissionManifest.permission).toHaveLength(1);
  expect(capability.permissions).toEqual([permission.identifier]);
  expect(permission.commands.deny).toEqual([]);
  expect([...allowedCommands].sort()).toEqual([...invokedCommands].sort());
});

test("uses only the Tauri core bridge", () => {
  const tauriPackages = new Set(
    Object.values(sourceFiles).flatMap((source) =>
      Array.from(
        source.matchAll(/["'](@tauri-apps\/[^"']+)["']/g),
        (match) => match[1],
      ),
    ),
  );

  expect([...tauriPackages]).toEqual(["@tauri-apps/api/core"]);
  expect(projectWindowCapability).not.toContain("dialog:");
});
