// @vitest-environment node
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, expect, test, vi } from "vitest";

import {
  LayoutExportBlockedError,
  MediaPreviewError,
  SaveProjectError,
} from "../application/projectPorts";
import { representativeProjection } from "../test/projectFixtures";
import { MediaExportBlockedError } from "../application/exportMedia";
import { ExportConflictsError } from "../application/normalExport";
import { StorageFullError } from "../application/storageRecovery";

import {
  tauriExportPipelinePort,
  tauriMediaPreviewPort,
  tauriWorkspacePreferencesPort,
  tauriProjectCorePort,
} from "./tauriProjectPorts";

const tauriBoundary = vi.hoisted(() => ({
  channels: [] as Array<{ onmessage: (message: unknown) => void }>,
}));
const eventBoundary = vi.hoisted(() => ({
  listeners: [] as Array<(event: { payload: unknown }) => void>,
}));

const exportSelection: Parameters<typeof tauriExportPipelinePort.startSheet>[0] = {
  options: { scope: "range", sheetIds: ["sheet-001"], mode: "sheet", format: { kind: "jpeg", quality: 100 }, destination: "C:/Exportados", conflictPolicy: "ask" },
  projectName: "Projeto de teste",
  sheetId: "sheet-001",
  sheetNumber: 1,
};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
  Channel: class<T> {
    onmessage = (_message: T) => undefined;

    constructor() {
      tauriBoundary.channels.push(
        this as unknown as { onmessage: (message: unknown) => void },
      );
    }
  },
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(
    async (_eventName: string, listener: (event: { payload: unknown }) => void) => {
      eventBoundary.listeners.push(listener);
      return vi.fn();
    },
  ),
}));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue(undefined);
  tauriBoundary.channels.length = 0;
  vi.mocked(listen).mockClear();
  eventBoundary.listeners.length = 0;
});

test("Layout queries send the requested total Frame count across the native boundary", async () => {
  await tauriProjectCorePort.queryLayouts("sheet-001", { frameCount: 2 });
  expect(invoke).toHaveBeenLastCalledWith("query_layouts", {
    sheetId: "sheet-001", frameRequest: { frameCount: 2 },
  });
  await tauriProjectCorePort.queryLayouts("sheet-001");
  expect(invoke).toHaveBeenLastCalledWith("query_layouts", { sheetId: "sheet-001" });
});

test("composes machine-local State with roaming Settings and routes updates to the owning store", async () => {
  const state = {
    inspectorSections: { "album.design": true },
    mediaThumbnailSize: 124,
    workspacePanels: {
      inspector: { size: 350, visible: true },
      media: null,
    },
  };
  const settings = {
    mediaPanel: {
      decorative: { sortKey: "name", sortDirection: "ascending", usageFilter: "all" },
      photo: { sortKey: "name", sortDirection: "descending", usageFilter: "used" },
    },
  };
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (
      command === "workspace_preferences" ||
      command === "update_workspace_preference"
    ) {
      return state;
    }
    return settings;
  });

  const preferences = {
    ...state,
    mediaPanelActiveKind: "photo",
    mediaPanel: settings.mediaPanel,
  };

  await expect(tauriWorkspacePreferencesPort.load()).resolves.toEqual(
    preferences,
  );
  await expect(
    tauriWorkspacePreferencesPort.update({
      kind: "inspectorSection",
      preferenceKey: "album.design",
      open: true,
    }),
  ).resolves.toEqual(preferences);
  await expect(
    tauriWorkspacePreferencesPort.update({
      kind: "mediaPanelSortDirection",
      mediaKind: "photo",
      sortDirection: "descending",
    }),
  ).resolves.toEqual(preferences);

  expect(invoke).toHaveBeenNthCalledWith(1, "workspace_preferences");
  expect(invoke).toHaveBeenNthCalledWith(2, "application_settings");
  expect(invoke).toHaveBeenNthCalledWith(3, "update_workspace_preference", {
    change: {
      kind: "inspectorSection",
      preferenceKey: "album.design",
      open: true,
    },
  });
  expect(invoke).toHaveBeenCalledWith("update_application_setting", {
    change: {
      kind: "mediaPanelSortDirection",
      mediaKind: "photo",
      sortDirection: "descending",
    },
  });
});

test("storage exhaustion preserves the native partial-publication warning", async () => {
  const message = "Alguns arquivos do álbum já foram exportados. Libere espaço e retome para concluir. Os arquivos já exportados foram mantidos.";
  vi.mocked(invoke).mockRejectedValueOnce({ code: "output_storage_full", message });
  const attempt = tauriExportPipelinePort.startSheet(exportSelection, vi.fn());
  await expect(attempt.completion).rejects.toBeInstanceOf(StorageFullError);
  await expect(attempt.completion).rejects.toThrow(message);
});

test("completes an Export attempt with the backend result", async () => {
  const result = {
    widthPx: 7_087,
    heightPx: 3_543,
  };
  vi.mocked(invoke).mockResolvedValueOnce(result);

  const attempt = tauriExportPipelinePort.startSheet(
    {
      ...exportSelection,
      projectName: "Álbum de teste",
      sheetId: "sheet-001",
      sheetNumber: 3,
    },
    vi.fn(),
  );

  await expect(attempt.completion).resolves.toEqual({
    status: "completed",
    result,
  });
  expect(invoke).toHaveBeenCalledWith("export_project", {
    options: exportSelection.options,
    onEvent: tauriBoundary.channels[0],
  });
});

test("native media preflight failures reach the recovery screen before any started event", async () => {
  const problems = [{ mediaId: "photo-1", fileName: "Foto.jpg", state: "absent" as const }];
  vi.mocked(invoke).mockRejectedValueOnce({ code: "media_problems", mediaProblems: problems });
  const event = vi.fn();
  const attempt = tauriExportPipelinePort.startSheet(exportSelection, event);
  await expect(attempt.completion).rejects.toEqual(new MediaExportBlockedError(problems));
  expect(event).not.toHaveBeenCalled();
});

test("unfilled Layout positions reach the export problems screen before any started event", async () => {
  const problems = [{ sheetId: "sheet-002", sheetNumber: 2, frameId: "frame-005", frameNumber: 3 }];
  vi.mocked(invoke).mockRejectedValueOnce({ code: "unfilled_layout_positions", layoutProblems: problems });
  const event = vi.fn();
  const attempt = tauriExportPipelinePort.startSheet(exportSelection, event);
  await expect(attempt.completion).rejects.toBeInstanceOf(LayoutExportBlockedError);
  await expect(attempt.completion).rejects.toMatchObject({ problems });
  expect(event).not.toHaveBeenCalled();
});

test.each([
  { name: "malformed", layoutProblems: [{ sheetId: "sheet-002", sheetNumber: 0, frameId: "frame-005", frameNumber: 3 }] },
  { name: "empty", layoutProblems: [] },
])("$name unfilled Layout positions keep the native failure instead of an empty problems screen", async ({ layoutProblems }) => {
  const failure = { code: "unfilled_layout_positions", layoutProblems };
  vi.mocked(invoke).mockRejectedValueOnce(failure);
  const attempt = tauriExportPipelinePort.startSheet(exportSelection, vi.fn());
  await expect(attempt.completion).rejects.toBe(failure);
});

test("resuming an interrupted export forwards its recovery id and a fresh export sends none", async () => {
  tauriExportPipelinePort.startSheet({ ...exportSelection, recoveryId: "recovery-7" }, vi.fn());
  expect(vi.mocked(invoke).mock.calls[0]).toStrictEqual(["export_project", {
    options: exportSelection.options, onEvent: tauriBoundary.channels[0], recoveryId: "recovery-7",
  }]);

  tauriExportPipelinePort.startSheet(exportSelection, vi.fn());
  expect(vi.mocked(invoke).mock.calls[1]).toStrictEqual(["export_project", {
    options: exportSelection.options, onEvent: tauriBoundary.channels[1],
  }]);
});

test("normal export sends the complete selection and maps overwrite conflicts without starting progress", async () => {
  const options = { scope: "range" as const, sheetIds: ["sheet-002", "sheet-003"], mode: "page" as const,
    format: { kind: "jpeg" as const, quality: 64 }, destination: "C:/álbuns/Exportados", conflictPolicy: "ask" as const };
  const conflicts = ["Álbum_003.jpg", "Álbum_004.jpg"];
  vi.mocked(invoke).mockRejectedValueOnce({ code: "export_conflict", conflicts });
  const event = vi.fn();
  const attempt = tauriExportPipelinePort.startSheet({ ...exportSelection, options }, event);
  await expect(attempt.completion).rejects.toEqual(new ExportConflictsError(conflicts));
  expect(invoke).toHaveBeenCalledWith("export_project", { options, onEvent: tauriBoundary.channels[0] });
  expect(event).not.toHaveBeenCalled();
});

test("an entirely skipped export completes without progress or fabricated output dimensions", async () => {
  const options = { scope: "album" as const, sheetIds: [exportSelection.sheetId], mode: "sheet" as const,
    format: { kind: "pdf" as const }, destination: "C:/Exportados", conflictPolicy: "skip" as const };
  vi.mocked(invoke).mockResolvedValueOnce(null);
  const event = vi.fn();
  const attempt = tauriExportPipelinePort.startSheet({ ...exportSelection, options }, event);
  await expect(attempt.completion).resolves.toEqual({ status: "skipped" });
  expect(event).not.toHaveBeenCalled();
  await expect(attempt.cancel()).resolves.toBe("not_found");
});

test("forwards Export events without exposing the backend operation id", () => {
  const onEvent = vi.fn();

  tauriExportPipelinePort.startSheet(exportSelection, onEvent);
  tauriBoundary.channels[0].onmessage({
    event: "started",
    data: {
      operationId: "export-42",
      cancellable: true,
    },
  });
  tauriBoundary.channels[0].onmessage({
    event: "progress",
    data: {
      operationId: "export-42",
      stage: "loading_sources",
      overallPercent: 4,
      units: {
        kind: "measured",
        completedUnits: 2,
        totalUnits: 5,
      },
      cancellable: true,
    },
  });

  expect(onEvent).toHaveBeenNthCalledWith(1, {
    event: "started",
    cancellable: true,
  });
  expect(onEvent).toHaveBeenNthCalledWith(2, {
    event: "progress",
    stage: "loading_sources",
    overallPercent: 4,
    units: {
      kind: "measured",
      completedUnits: 2,
      totalUnits: 5,
    },
    cancellable: true,
  });
});

test("cancels an Export attempt using the operation id kept inside the adapter", async () => {
  vi.mocked(invoke).mockImplementationOnce(
    () => new Promise(() => undefined),
  );
  vi.mocked(invoke).mockResolvedValueOnce("requested");

  const attempt = tauriExportPipelinePort.startSheet(exportSelection, vi.fn());
  tauriBoundary.channels[0].onmessage({
    event: "started",
    data: {
      operationId: "export-42",
      cancellable: true,
    },
  });

  const firstCancellation = attempt.cancel();
  const repeatedCancellation = attempt.cancel();

  await expect(firstCancellation).resolves.toBe("requested");
  await expect(repeatedCancellation).resolves.toBe("requested");
  expect(invoke).toHaveBeenNthCalledWith(2, "cancel_export", {
    operationId: "export-42",
  });
  expect(invoke).toHaveBeenCalledTimes(2);
});

test("maps the backend cancelled error to a cancelled Export outcome", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({
    code: "cancelled",
    message: "A exportação foi cancelada.",
  });

  const attempt = tauriExportPipelinePort.startSheet(exportSelection, vi.fn());

  await expect(attempt.completion).resolves.toEqual({
    status: "cancelled",
  });
});

test("keeps a cancellation requested before started until the operation id arrives", async () => {
  let completeBackend: (value: unknown) => void = () => undefined;
  vi.mocked(invoke).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        completeBackend = resolve;
      }),
  );
  vi.mocked(invoke).mockResolvedValueOnce("requested");

  const attempt = tauriExportPipelinePort.startSheet(exportSelection, vi.fn());
  const cancellation = attempt.cancel();
  await Promise.resolve();

  expect(invoke).toHaveBeenCalledTimes(1);

  tauriBoundary.channels[0].onmessage({
    event: "started",
    data: {
      operationId: "export-delayed",
      cancellable: true,
    },
  });

  await expect(cancellation).resolves.toBe("requested");
  expect(invoke).toHaveBeenNthCalledWith(2, "cancel_export", {
    operationId: "export-delayed",
  });

  completeBackend({
    widthPx: 7_087,
    heightPx: 3_543,
  });
  await attempt.completion;
});

test("resolves a queued cancellation as not_found when completion fails before started", async () => {
  const failure = {
    code: "conflict",
    message: "Outra operação exclusiva já está em andamento.",
  };
  vi.mocked(invoke).mockRejectedValueOnce(failure);

  const attempt = tauriExportPipelinePort.startSheet(exportSelection, vi.fn());
  const cancellation = attempt.cancel();

  await expect(attempt.completion).rejects.toBe(failure);
  await expect(cancellation).resolves.toBe("not_found");
  expect(invoke).toHaveBeenCalledTimes(1);
});

test("materializes an owned media-demand DTO at the native seam", async () => {
  vi.mocked(invoke).mockResolvedValueOnce([]);
  const visibleMediaIds = Object.freeze(["media-a-001"]);
  const preloadMediaIds = Object.freeze(["media-b-001"]);

  await tauriMediaPreviewPort.prepareMediaPreviews({
    preloadMediaIds,
    revision: 7,
    visibleMediaIds,
  }, vi.fn());

  const request = vi.mocked(invoke).mock.calls[0][1] as {
    demand: {
      preloadMediaIds: string[];
      revision: number;
      visibleMediaIds: string[];
    };
  };
  expect(request.demand).toEqual({
    preloadMediaIds: ["media-b-001"],
    revision: 7,
    visibleMediaIds: ["media-a-001"],
  });
  expect(request.demand.visibleMediaIds).not.toBe(visibleMediaIds);
  expect(request.demand.preloadMediaIds).not.toBe(preloadMediaIds);
});


test.each(["completed", "failed"])("streams each media preview before the batch finishes and ignores late events after %s", async (outcome) => {
  let finish!: () => void;
  vi.mocked(invoke).mockImplementationOnce(() => new Promise((resolve, reject) => {
    finish = () => outcome === "completed" ? resolve([]) : reject(new Error("Falhou"));
  }));
  const publish = vi.fn();
  const demand = { revision: 1, visibleMediaIds: ["photo-a", "photo-b"], preloadMediaIds: [] };
  const completion = tauriMediaPreviewPort.prepareMediaPreviews(demand, publish).catch(() => undefined);
  const channel = tauriBoundary.channels[0];
  expect(invoke).toHaveBeenCalledWith("prepare_media_previews", { demand, onPreview: channel });
  const preview = { mediaId: "photo-a", state: "ready", url: "http://myalbuns-cache.localhost/a" };
  channel.onmessage(preview);
  expect(publish).toHaveBeenCalledExactlyOnceWith(preview);
  finish();
  await completion;
  channel.onmessage({ ...preview, mediaId: "photo-b" });
  expect(publish).toHaveBeenCalledOnce();
});

test.each(["completed", "failed"])("streams photo import progress per attempt and ignores late events after %s", async (outcome) => {
  let finish!: () => void;
  vi.mocked(invoke).mockImplementationOnce(() => new Promise<void>((resolve, reject) => {
    finish = () => outcome === "completed" ? resolve() : reject(new Error("Falhou"));
  }));
  const onProgress = vi.fn();
  const selection = { mediaKind: "decorative" as const, source: { kind: "folder" as const } };
  const completion = tauriProjectCorePort.importMedia(onProgress, selection).catch(() => undefined);
  const channel = tauriBoundary.channels[0];
  expect(invoke).toHaveBeenCalledWith("import_media", { selection, onProgress: channel });
  expect(onProgress).not.toHaveBeenCalled();
  channel.onmessage({ completedFiles: 0, totalFiles: 12 });
  channel.onmessage({ completedFiles: 5, totalFiles: 12 });
  expect(onProgress.mock.calls).toEqual([[{ completedFiles: 0, totalFiles: 12 }], [{ completedFiles: 5, totalFiles: 12 }]]);
  finish();
  await completion;
  channel.onmessage({ completedFiles: 12, totalFiles: 12 });
  expect(onProgress).toHaveBeenCalledTimes(2);
});

test("maps stable linked-media events to the reactive preview seam", async () => {
  const listener = vi.fn();

  const unlisten = await tauriMediaPreviewPort.onMediaChanged(listener);
  eventBoundary.listeners[0]({
    payload: { mediaIds: ["photo-a", "overlay-a"] },
  });

  expect(listen).toHaveBeenCalledWith(
    "myalbuns://linked-media-changed",
    expect.any(Function),
  );
  expect(listener).toHaveBeenCalledWith(["photo-a", "overlay-a"]);
  expect(unlisten).toEqual(expect.any(Function));
});

test("returns the authoritative projection from a confirmed Project save", async () => {
  const savedProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      savedRevision: 25,
      dirty: false,
      canUndo: true,
    },
  };
  const result = {
    outcome: { kind: "saved" as const, revision: 25 },
    projection: savedProjection,
  };
  vi.mocked(invoke).mockResolvedValueOnce(result);

  await expect(tauriProjectCorePort.save(25)).resolves.toEqual(
    result,
  );
  expect(invoke).toHaveBeenCalledWith("save_project", {
    expectedRevision: 25,
  });
});

test("only a confirmed format conversion tells the native save to replace an old myAlbuns file", async () => {
  const result = {
    outcome: { kind: "saved" as const, revision: 25 },
    projection: {
      ...representativeProjection,
      state: { ...representativeProjection.state, savedRevision: 25, dirty: false },
    },
  };
  vi.mocked(invoke).mockResolvedValue(result);

  await expect(tauriProjectCorePort.save(25, true)).resolves.toEqual(result);
  await tauriProjectCorePort.save(25);
  await tauriProjectCorePort.save(25, false);

  // Strict: an explicit `confirmFormatConversion: false` or `undefined`
  // key would still be a different payload from the plain save.
  expect(vi.mocked(invoke).mock.calls).toStrictEqual([
    ["save_project", { expectedRevision: 25, confirmFormatConversion: true }],
    ["save_project", { expectedRevision: 25 }],
    ["save_project", { expectedRevision: 25 }],
  ]);
});

test("invokes the native Salvar como flow and validates the adopted Project", async () => {
  const savedAsProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      projectId: "81f68858-c8f5-4fcb-8e0f-185c3ff45cf5",
      projectName: "Versão independente",
      savedRevision: 25,
      dirty: false,
      canUndo: true,
    },
  };
  const result = {
    outcome: {
      kind: "savedAs" as const,
      previousProjectId: representativeProjection.state.projectId,
      projectId: savedAsProjection.state.projectId,
      revision: 25,
    },
    projection: savedAsProjection,
  };
  vi.mocked(invoke).mockResolvedValueOnce(result);

  await expect(tauriProjectCorePort.saveAs(25)).resolves.toEqual(result);
  expect(invoke).toHaveBeenCalledWith("save_project_as", {
    expectedRevision: 25,
  });
});

test("accepts native Salvar como cancellation without changing the projection", async () => {
  const result = {
    outcome: { kind: "cancelled" as const },
    projection: representativeProjection,
  };
  vi.mocked(invoke).mockResolvedValueOnce(result);

  await expect(tauriProjectCorePort.saveAs(25)).resolves.toEqual(result);
});

test("rejects a Salvar como cancellation whose projection is not the requested visible revision", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    outcome: { kind: "cancelled" },
    projection: {
      ...representativeProjection,
      state: { ...representativeProjection.state, revision: 26 },
    },
  });

  const failure = tauriProjectCorePort.saveAs(25);

  await expect(failure).rejects.toBeInstanceOf(SaveProjectError);
  await expect(failure).rejects.toMatchObject({
    code: "invalid_response",
    message: "Não foi possível confirmar o resultado de Salvar como.",
  });
});

test.each([
  { name: "an unknown code", wire: { code: "not_a_save_as_failure" } },
  { name: "a non-structured rejection", wire: new Error("IPC channel closed") },
])("maps $name from Salvar como to an unavailable save without leaking diagnostics", async ({ wire }) => {
  vi.mocked(invoke).mockRejectedValueOnce(wire);

  const failure = tauriProjectCorePort.saveAs(25);

  await expect(failure).rejects.toBeInstanceOf(SaveProjectError);
  await expect(failure).rejects.toMatchObject({
    code: "save_unavailable",
    message: "Não foi possível iniciar Salvar como.",
  });
});

test("rejects Salvar como when the adopted identity and projection disagree", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    outcome: {
      kind: "savedAs",
      previousProjectId: representativeProjection.state.projectId,
      projectId: "81f68858-c8f5-4fcb-8e0f-185c3ff45cf5",
      revision: 25,
    },
    projection: representativeProjection,
  });

  await expect(tauriProjectCorePort.saveAs(25)).rejects.toMatchObject({
    code: "invalid_response",
    message: "Não foi possível confirmar o resultado de Salvar como.",
  });
});

test("rejects Salvar como identities that are not canonical Project UUIDs", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    outcome: {
      kind: "savedAs",
      previousProjectId: representativeProjection.state.projectId,
      projectId: "not-a-project-identity",
      revision: 25,
    },
    projection: {
      ...representativeProjection,
      state: {
        ...representativeProjection.state,
        projectId: "not-a-project-identity",
        savedRevision: 25,
        dirty: false,
      },
    },
  });

  await expect(tauriProjectCorePort.saveAs(25)).rejects.toMatchObject({
    code: "invalid_response",
  });
});

test("rejects a Salvar como revision different from the requested visible revision", async () => {
  const copiedProjectId = "81f68858-c8f5-4fcb-8e0f-185c3ff45cf5";
  vi.mocked(invoke).mockResolvedValueOnce({
    outcome: {
      kind: "savedAs",
      previousProjectId: representativeProjection.state.projectId,
      projectId: copiedProjectId,
      revision: 24,
    },
    projection: {
      ...representativeProjection,
      state: {
        ...representativeProjection.state,
        projectId: copiedProjectId,
        revision: 24,
        savedRevision: 24,
        dirty: false,
      },
    },
  });

  await expect(tauriProjectCorePort.saveAs(25)).rejects.toMatchObject({
    code: "invalid_response",
  });
});

test("maps an indeterminate Salvar como terminal without hiding destination risk", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({
    code: "save_as_state_indeterminate",
  });

  await expect(tauriProjectCorePort.saveAs(25)).rejects.toMatchObject({
    code: "save_as_state_indeterminate",
    message:
      "Não foi possível confirmar se a cópia foi salva. O projeto anterior continua aberto. Confira o arquivo no destino escolhido antes de tentar novamente.",
  });
});

test("accepts an already-current Project save envelope", async () => {
  const currentProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      savedRevision: 25,
      dirty: false,
    },
  };
  const result = {
    outcome: { kind: "alreadyCurrent" as const, revision: 25 },
    projection: currentProjection,
  };
  vi.mocked(invoke).mockResolvedValueOnce(result);

  await expect(tauriProjectCorePort.save(25)).resolves.toEqual(
    result,
  );
});

test("rejects a Project save envelope whose projection does not confirm its outcome", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    outcome: { kind: "saved", revision: 25 },
    projection: {
      ...representativeProjection,
      state: {
        ...representativeProjection.state,
        savedRevision: 24,
      },
    },
  });

  await expect(tauriProjectCorePort.save(25)).rejects.toMatchObject({
    code: "invalid_response",
    message: "Não foi possível confirmar o resultado do salvamento.",
  });
});

test("rejects malformed stale-revision context as an unavailable save", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({
    code: "stale_revision",
    expectedRevision: "24",
    currentRevision: 25,
  });

  await expect(tauriProjectCorePort.save(25)).rejects.toMatchObject({
    code: "save_unavailable",
    message: "Não foi possível iniciar o salvamento do projeto.",
  });
});

test.each([
  {
    wire: {
      code: "stale_revision",
      expectedRevision: 24,
      currentRevision: 25,
    },
    code: "stale_revision",
    message:
      "Não foi possível salvar porque há alterações mais recentes no projeto. Nada foi salvo nesta tentativa.",
    context: { expected: 24, current: 25 },
  },
  {
    wire: { code: "persisted_baseline_conflict" },
    code: "persisted_baseline_conflict",
    message:
      "O arquivo do projeto foi alterado fora do MyAlbuns. O salvamento não substituiu essas alterações.",
  },
  {
    wire: { code: "io_failure" },
    code: "io_failure",
    message: "O Windows não conseguiu concluir o salvamento do projeto.",
  },
] as const)(
  "localizes the structured $code Project save failure",
  async ({ wire, code, message, ...expected }) => {
    vi.mocked(invoke).mockRejectedValueOnce(wire);

    const failure = tauriProjectCorePort.save(25);

    await expect(failure).rejects.toBeInstanceOf(SaveProjectError);
    await expect(failure).rejects.toMatchObject({
      code,
      message,
      ...expected,
    });
  },
);

test("normalizes typed media preview failures without losing their code or message", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({
    code: "unavailable",
    message: "A imagem decorativa vinculada não está disponível.",
  });

  const failure = tauriMediaPreviewPort.prepareMediaPreviews({
    revision: 1,
    visibleMediaIds: ["media-a-001"],
    preloadMediaIds: [],
  }, vi.fn());

  await expect(failure).rejects.toBeInstanceOf(MediaPreviewError);
  await expect(failure).rejects.toMatchObject({
    code: "unavailable",
    message: "A imagem decorativa vinculada não está disponível.",
  });
});

test("normalizes typed unavailable-media retry failures at the IPC adapter", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({
    code: "read_failed",
    message: "A nova inspeção não pôde ser concluída.",
  });

  const failure = tauriMediaPreviewPort.retryUnavailableMedia("media-a-001", vi.fn());

  await expect(failure).rejects.toBeInstanceOf(MediaPreviewError);
  await expect(failure).rejects.toMatchObject({
    code: "read_failed",
    message: "A nova inspeção não pôde ser concluída.",
  });
});
