import { expect, test } from "vitest";
import globalWindowCapability from "../../src-tauri/capabilities/global.json?raw";
import projectWindowCapability from "../../src-tauri/capabilities/default.json?raw";
import globalWindowPermission from "../../src-tauri/permissions/global-window.json?raw";
import projectWindowPermission from "../../src-tauri/permissions/project-window.json?raw";
import productRuntimeSource from "../../src-tauri/src/product_runtime.rs?raw";
import projectCommandsSource from "../../src-tauri/src/project_commands.rs?raw";

const sourceFiles = import.meta.glob("../**/*.{ts,tsx}", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

const tauriCommandSources = {
  shared: ["./tauriLogger.ts"],
  project: ["./tauriProjectPorts.ts", "./tauriProjectWindowPort.ts"],
  global: ["../global/platform/tauriGlobalProjectPort.ts"],
} as const;

const compositionRoots = new Set(["../main.tsx", "../global/main.tsx"]);
const platformDirectories = ["../platform/", "../global/platform/"];
const issue16GlobalCacheCommands = new Set([
  "cache_service_status",
  "free_closed_project_cache",
  "clear_all_cache",
]);

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

function parseSurfaceContract(capabilitySource: string, permissionSource: string) {
  const capability = JSON.parse(capabilitySource) as {
    windows: string[];
    permissions: string[];
  };
  const permissionManifest = JSON.parse(permissionSource) as {
    permission: Array<{
      identifier: string;
      commands: { allow: string[]; deny: string[] };
    }>;
  };

  expect(permissionManifest.permission).toHaveLength(1);
  const permission = permissionManifest.permission[0];
  expect(
    capability.permissions.filter((identifier) =>
      identifier.endsWith("-window-commands"),
    ),
  ).toEqual([permission.identifier]);
  expect(permission.commands.deny).toEqual([]);

  return { capability, allowedCommands: new Set(permission.commands.allow) };
}

test("keeps Tauri dependencies inside platform adapters", () => {
  const tauriPackagePrefix = ["@tauri", "-apps"].join("");
  const offenders = findOffenders(
    (path, source) =>
      source.includes(tauriPackagePrefix) &&
      !path.startsWith("./") &&
      !platformDirectories.some((directory) => path.startsWith(directory)),
  );

  expect(offenders).toEqual([]);
});

test("selects concrete platform adapters only at the composition root", () => {
  const platformImport =
    /(?:from\s+|import\s*)["'][^"']*\/platform\//;
  const offenders = findOffenders(
    (path, source) =>
      !compositionRoots.has(path) &&
      !platformDirectories.some((directory) => path.startsWith(directory)) &&
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
    ...tauriCommandSources.global,
  ];

  expect(new Set(assignedSources).size).toBe(assignedSources.length);
  expect([...assignedSources].sort()).toEqual(invokingSources);
});

test("keeps the project-window capability aligned with the invoked commands", () => {
  const invokedCommands = extractInvokedCommands(
    [...tauriCommandSources.shared, ...tauriCommandSources.project],
  );
  const { capability, allowedCommands } = parseSurfaceContract(
    projectWindowCapability,
    projectWindowPermission,
  );

  expect(capability.windows).toEqual(["project"]);
  expect(capability.permissions).toEqual([
    "project-window-commands",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
  ]);
  expect([...allowedCommands].sort()).toEqual([...invokedCommands].sort());
});

test("keeps the global-window capability isolated from project commands", () => {
  const globalCommands = extractInvokedCommands(tauriCommandSources.global);
  const explicitGlobalSurface = new Set([
    ...globalCommands,
    ...issue16GlobalCacheCommands,
  ]);
  const projectCommands = extractInvokedCommands([
    ...tauriCommandSources.shared,
    ...tauriCommandSources.project,
  ]);
  const { capability, allowedCommands } = parseSurfaceContract(
    globalWindowCapability,
    globalWindowPermission,
  );

  expect(capability.windows).toEqual(["global"]);
  expect([...allowedCommands].sort()).toEqual(
    [...explicitGlobalSurface].sort(),
  );
  expect(
    [...allowedCommands].filter((command) => projectCommands.has(command)),
  ).toEqual([]);
});

test("uses only the minimal Tauri core and event bridges", () => {
  const tauriPackages = new Set(
    Object.values(sourceFiles).flatMap((source) =>
      Array.from(
        source.matchAll(/["'](@tauri-apps\/[^"']+)["']/g),
        (match) => match[1],
      ),
    ),
  );

  expect([...tauriPackages].sort()).toEqual([
    "@tauri-apps/api/core",
    "@tauri-apps/api/event",
  ]);
  expect(projectWindowCapability).not.toContain("dialog:");
});

test("consumes the generated import result at the Tauri boundary", () => {
  const projectPortSource = sourceFiles["./tauriProjectPorts.ts"];

  expect(projectPortSource).toContain(
    'import type { ImportPhotoResult as IpcImportPhotoResult } from "./generated/ImportPhotoResult";',
  );
  expect(projectPortSource).toContain(
    'invoke<IpcImportPhotoResult>("import_photo")',
  );
});

test("initializes the native dialog used by the productive relink command", () => {
  expect(projectCommandsSource).toContain("app.dialog()");
  expect(productRuntimeSource).toContain(
    ".plugin(tauri_plugin_dialog::init())",
  );
  expect(projectWindowCapability).not.toContain("dialog:");
});
