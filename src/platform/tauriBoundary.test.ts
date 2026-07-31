import { expect, test } from "vitest";
import projectWindowCapability from "../../src-tauri/capabilities/default.json?raw";
import projectWindowPermission from "../../src-tauri/permissions/project-window.json?raw";

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

test("keeps the project-window capability aligned with the invoked commands", () => {
  const invokedCommands = new Set(
    Object.entries(sourceFiles)
      .filter(
        ([path, source]) =>
          !path.includes(".test.") &&
          source.includes("@tauri-apps/api/core"),
      )
      .flatMap(([, source]) =>
        Array.from(
          source.matchAll(/\binvoke(?:<[^>]+>)?\(\s*["']([^"']+)["']/g),
          (match) => match[1],
        ),
      ),
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

test("does not expose generic filesystem or shell packages to the frontend", () => {
  const tauriPackages = new Set(
    Object.values(sourceFiles).flatMap((source) =>
      Array.from(
        source.matchAll(/["'](@tauri-apps\/[^"']+)["']/g),
        (match) => match[1],
      ),
    ),
  );

  expect([...tauriPackages]).toEqual(["@tauri-apps/api/core"]);
});
