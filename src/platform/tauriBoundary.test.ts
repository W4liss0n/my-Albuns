import { expect, test } from "vitest";
import globalWindowCapability from "../../src-tauri/capabilities/global.json?raw";
import dialogWindowCapability from "../../src-tauri/capabilities/dialog.json?raw";
import progressDialogWindowCapability from "../../src-tauri/capabilities/dialog-progress.json?raw";
import projectDialogWindowCapability from "../../src-tauri/capabilities/project-dialog.json?raw";
import projectWindowCapability from "../../src-tauri/capabilities/default.json?raw";
import globalWindowPermission from "../../src-tauri/permissions/global-window.json?raw";
import messageDialogWindowPermission from "../../src-tauri/permissions/message-dialog-window.json?raw";
import ownedDialogWindowPermission from "../../src-tauri/permissions/owned-dialog-window.json?raw";
import projectWindowPermission from "../../src-tauri/permissions/project-window.json?raw";
import projectDialogWindowPermission from "../../src-tauri/permissions/project-dialog-window.json?raw";

const sourceFiles = import.meta.glob("../**/*.{ts,tsx}", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

const tauriCommandSources = {
  shared: ["./tauriLogger.ts"],
  ownedDialog: ["./tauriWindowControls.ts"],
  messageDialog: ["./tauriOwnedDialogControls.ts"],
  project: [
    "./tauriProjectDialogPort.ts",
    "./tauriProjectPorts.ts",
    "./tauriProjectWindowPort.ts",
  ],
  projectDialog: [
    "../project-dialog/platform/tauriProjectDialogClient.ts",
  ],
  global: [
    "../global/platform/tauriGlobalProjectPort.ts",
    "../global/platform/tauriNewProjectPort.ts",
  ],
} as const;

const compositionRoots = new Set([
  "../dialog/main.tsx",
  "../global/main.tsx",
  "../main.tsx",
  "../project-dialog/main.tsx",
]);
const platformDirectories = [
  "../platform/",
  "../global/platform/",
  "../project-dialog/platform/",
];

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
    ...tauriCommandSources.ownedDialog,
    ...tauriCommandSources.messageDialog,
    ...tauriCommandSources.project,
    ...tauriCommandSources.projectDialog,
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
    "core:window:allow-close",
    "core:window:allow-minimize",
    "core:window:allow-toggle-maximize",
    "core:window:allow-start-dragging",
    "core:window:allow-internal-toggle-maximize",
  ]);
  expect([...allowedCommands].sort()).toEqual([...invokedCommands].sort());
});

test("keeps the global-window capability isolated from project commands", () => {
  const globalCommands = extractInvokedCommands(tauriCommandSources.global);
  const projectCommands = extractInvokedCommands([
    ...tauriCommandSources.shared,
    ...tauriCommandSources.project,
  ]);
  const { capability, allowedCommands } = parseSurfaceContract(
    globalWindowCapability,
    globalWindowPermission,
  );

  expect(capability.windows).toEqual(["global"]);
  expect([...allowedCommands].sort()).toEqual([...globalCommands].sort());
  expect(
    [...allowedCommands].filter((command) => projectCommands.has(command)),
  ).toEqual([]);
});

test("limits the Project dialog to state hydration and semantic actions", () => {
  const invokedCommands = extractInvokedCommands(
    [
      ...tauriCommandSources.projectDialog,
      ...tauriCommandSources.ownedDialog,
    ],
  );
  const { capability, allowedCommands } = parseSurfaceContract(
    projectDialogWindowCapability,
    projectDialogWindowPermission,
  );

  expect(capability.windows).toEqual(["project-dialog"]);
  expect(capability.permissions).toEqual([
    "project-dialog-window-commands",
    "core:event:allow-listen",
    "core:event:allow-unlisten",
    "core:window:allow-close",
    "core:window:allow-center",
    "core:window:allow-set-size",
    "core:window:allow-start-dragging",
  ]);
  expect([...allowedCommands].sort()).toEqual([...invokedCommands].sort());
});

test("gives each standard dialog only the abilities exposed by its titlebar", () => {
  const messageInvokedCommands = extractInvokedCommands(
    [
      ...tauriCommandSources.ownedDialog,
      ...tauriCommandSources.messageDialog,
    ],
  );
  const progressInvokedCommands = extractInvokedCommands(
    tauriCommandSources.ownedDialog,
  );
  const {
    capability: messageCapability,
    allowedCommands: messageCommands,
  } = parseSurfaceContract(
    dialogWindowCapability,
    messageDialogWindowPermission,
  );
  const {
    capability: progressCapability,
    allowedCommands: progressCommands,
  } = parseSurfaceContract(
    progressDialogWindowCapability,
    ownedDialogWindowPermission,
  );

  expect(messageCapability.windows).toEqual(["dialog-project-failure"]);
  expect(messageCapability.permissions).toEqual([
    "message-dialog-window-commands",
    "core:window:allow-center",
    "core:window:allow-set-size",
    "core:window:allow-start-dragging",
  ]);
  expect(progressCapability.windows).toEqual(["dialog-opening-progress"]);
  expect(progressCapability.permissions).toEqual([
    "owned-dialog-window-commands",
    "core:window:allow-center",
    "core:window:allow-set-size",
    "core:window:allow-start-dragging",
  ]);
  expect([...messageCommands].sort()).toEqual(
    [...messageInvokedCommands].sort(),
  );
  expect([...progressCommands].sort()).toEqual(
    [...progressInvokedCommands].sort(),
  );
});

test("uses only the minimal Tauri core, event, and window bridges", () => {
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
    "@tauri-apps/api/window",
  ]);
  expect(projectWindowCapability).not.toContain("dialog:");
});
