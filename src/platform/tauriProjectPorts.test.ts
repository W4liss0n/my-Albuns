import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { beforeEach, expect, test, vi } from "vitest";

import type { ProjectIntent } from "../domain/project";
import {
  MediaPreviewError,
  SaveProjectError,
} from "../application/projectPorts";
import { representativeProjection } from "../test/projectFixtures";
import {
  tauriExportPipelinePort,
  tauriMediaPreviewPort,
  tauriWorkspacePreferencesPort,
  tauriProjectCorePort,
  tauriProjectStartupPort,
} from "./tauriProjectPorts";

const tauriBoundary = vi.hoisted(() => ({
  channels: [] as Array<{ onmessage: (message: unknown) => void }>,
}));
const eventBoundary = vi.hoisted(() => ({
  listeners: [] as Array<(event: { payload: unknown }) => void>,
}));

const exportSelection = {
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

test("composes machine-local State with roaming Settings and routes updates to the owning store", async () => {
  const state = {
    inspectorSections: { "album.design": true },
    mediaThumbnailSizes: { decorative: 110, photo: 124 },
    workspacePanels: {
      inspector: { size: 350, visible: true },
      media: null,
    },
  };
  const settings = {
    mediaPanel: {
      decorative: { sortDirection: "ascending", usageFilter: "all" },
      photo: { sortDirection: "descending", usageFilter: "used" },
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

test("completes an Export attempt with the backend result", async () => {
  const result = {
    widthPx: 7_087,
    heightPx: 3_543,
  };
  vi.mocked(invoke).mockResolvedValueOnce(result);

  const attempt = tauriExportPipelinePort.startSheet(
    {
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
  expect(invoke).toHaveBeenCalledWith("export_sheet", {
    projectName: "Álbum de teste",
    sheetId: "sheet-001",
    sheetNumber: 3,
    onEvent: tauriBoundary.channels[0],
  });
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
    message: "A Exportação foi cancelada.",
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

test("maps the Project and media ports to the desktop commands", async () => {
  const information = {
    displayUnit: "mm" as const,
    sheetWidthUm: 600_000,
    sheetHeightUm: 300_000,
    dpi: 300,
    bleedUm: 3_000,
    safetyUm: 3_000,
    firstSheet: "double" as const,
    lastSheet: "double" as const,
  };
  const intent: ProjectIntent = {
    kind: "addPhoto",
    sheetId: "sheet-002",
    mediaId: "media-campo",
    mode: "normal",
  };

  vi.mocked(invoke)
    .mockResolvedValueOnce(representativeProjection)
    .mockResolvedValueOnce({
      errors: [],
      impact: { heightPx: 3_543, pageWidthPx: 3_543, sheetWidthPx: 7_087 },
    })
    .mockResolvedValueOnce({
      projection: representativeProjection,
      affectedFrameId: "frame-001",
      affectedSheetId: null,
    });
  await tauriProjectCorePort.load("project-load-1");
  await tauriProjectCorePort.validateAlbumInformation(information);
  await tauriProjectCorePort.apply(intent);
  await tauriProjectCorePort.relink("media-a-001");
  await tauriProjectCorePort.undo();
  await tauriProjectCorePort.redo();
  const retriedPreview = {
    mediaId: "media-a-001",
    state: "unavailable" as const,
    url: "http://asset.localhost/last-cache-preview",
  };
  vi.mocked(invoke).mockResolvedValueOnce(retriedPreview);
  const retry = await tauriMediaPreviewPort.retryUnavailableMedia(
    "media-a-001",
  );
  vi.mocked(invoke).mockResolvedValueOnce([
    {
      mediaId: "media-a-001",
      state: "ready",
      url: "http://asset.localhost/cache-preview",
    },
  ]);
  const demand = {
    revision: 1,
    visibleMediaIds: ["media-a-001"],
    preloadMediaIds: ["media-b-001"],
  };
  const previews = await tauriMediaPreviewPort.prepareMediaPreviews(demand);

  expect(invoke).toHaveBeenNthCalledWith(1, "project_state", {
    operationId: "project-load-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "validate_album_information", {
    information,
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "apply_project_intent", {
    intent,
  });
  expect(invoke).toHaveBeenNthCalledWith(4, "relink_media", {
    mediaId: "media-a-001",
  });
  expect(invoke).toHaveBeenNthCalledWith(5, "undo_project");
  expect(invoke).toHaveBeenNthCalledWith(6, "redo_project");
  expect(invoke).toHaveBeenNthCalledWith(7, "retry_unavailable_media", {
    mediaId: "media-a-001",
  });
  expect(invoke).toHaveBeenNthCalledWith(8, "prepare_media_previews", {
    demand,
  });
  expect(retry).toEqual(retriedPreview);
  expect(previews?.[0].url).toBe("http://asset.localhost/cache-preview");
});

test("materializes an owned media-demand DTO at the native seam", async () => {
  vi.mocked(invoke).mockResolvedValueOnce([]);
  const visibleMediaIds = Object.freeze(["media-a-001"]);
  const preloadMediaIds = Object.freeze(["media-b-001"]);

  await tauriMediaPreviewPort.prepareMediaPreviews({
    preloadMediaIds,
    revision: 7,
    visibleMediaIds,
  });

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

test("maps one closed Recovery decision without a separate confirmation flag", async () => {
  vi.mocked(invoke)
    .mockResolvedValueOnce({ kind: "available" })
    .mockResolvedValueOnce({
      kind: "openedLastSaved",
      projection: representativeProjection,
    });

  await expect(tauriProjectStartupPort.recoveryStatus()).resolves.toEqual({
    kind: "available",
  });
  await expect(
    tauriProjectStartupPort.resolveRecovery(
      "discardCheckpointAndOpenLastSaved",
    ),
  ).resolves.toEqual({
    kind: "openedLastSaved",
    projection: representativeProjection,
  });

  expect(invoke).toHaveBeenNthCalledWith(1, "project_recovery_status");
  expect(invoke).toHaveBeenNthCalledWith(2, "resolve_project_recovery", {
    decision: "discardCheckpointAndOpenLastSaved",
  });
});

test("rejects an open Recovery response outside the generated contract", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ kind: "futureChoice" });

  await expect(tauriProjectStartupPort.recoveryStatus()).rejects.toThrow(
    "Não foi possível verificar a Recuperação do Projeto.",
  );
});

test("rejects extra fields in the closed Recovery status and resolution envelopes", async () => {
  vi.mocked(invoke)
    .mockResolvedValueOnce({ kind: "available", futureField: true })
    .mockResolvedValueOnce({ kind: "deferred", projection: representativeProjection });

  await expect(tauriProjectStartupPort.recoveryStatus()).rejects.toThrow(
    "Não foi possível verificar a Recuperação do Projeto.",
  );
  await expect(
    tauriProjectStartupPort.resolveRecovery("nowNot"),
  ).rejects.toThrow("Não foi possível confirmar a escolha de Recuperação.");
});

test("maps Photo import, target resolution, and affected Frame outcomes", async () => {
  const mutationOutcome = {
    projection: representativeProjection,
    affectedFrameId: "frame-001",
    affectedSheetId: null,
  };
  const importOutcome = {
    kind: "imported" as const,
    projection: representativeProjection,
    mediaId: "media-imported",
  };
  vi.mocked(invoke)
    .mockResolvedValueOnce(mutationOutcome)
    .mockResolvedValueOnce(importOutcome)
    .mockResolvedValueOnce({ kind: "sheet", sheetId: "sheet-001" });
  const intent: ProjectIntent = {
    kind: "addPhoto",
    sheetId: "sheet-001",
    mediaId: "media-imported",
    mode: "normal",
  };

  await expect(
    tauriProjectCorePort.applyWithOutcome(intent),
  ).resolves.toEqual(mutationOutcome);
  await expect(tauriProjectCorePort.importPhoto()).resolves.toEqual(
    importOutcome,
  );
  await expect(
    tauriProjectCorePort.resolvePhotoDropTarget(
      "sheet-001",
      12_000,
      34_000,
    ),
  ).resolves.toEqual({ kind: "sheet", sheetId: "sheet-001" });

  expect(invoke).toHaveBeenNthCalledWith(1, "apply_project_intent", {
    intent,
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "import_photo");
  expect(invoke).toHaveBeenNthCalledWith(3, "photo_drop_target", {
    sheetId: "sheet-001",
    xUm: 12_000,
    yUm: 34_000,
  });
  expect(vi.mocked(invoke).mock.calls[2]?.[1]).not.toHaveProperty("mode");
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

test("maps the typed Cache processor warning without blocking Project commands", async () => {
  const listener = vi.fn();

  const unlisten = await tauriMediaPreviewPort.onCacheProcessorWarning(listener);
  eventBoundary.listeners[0]({
    payload: {
      state: "suspended",
      message: "O Cache foi suspenso após falhas repetidas.",
    },
  });

  expect(listen).toHaveBeenCalledWith(
    "myalbuns://cache-processor-warning",
    expect.any(Function),
  );
  expect(listener).toHaveBeenCalledWith({
    state: "suspended",
    message: "O Cache foi suspenso após falhas repetidas.",
  });
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
      "Não foi possível confirmar o destino de Salvar como. A Sessão anterior foi mantida; reinspecione o destino antes de reutilizá-lo.",
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
    message: "Não foi possível confirmar o resultado do Salvamento.",
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
    message: "Não foi possível iniciar o Salvamento do Projeto.",
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
      "A revisão visível ficou desatualizada. Atualize o Projeto e tente salvar novamente.",
    context: { expected: 24, current: 25 },
  },
  {
    wire: { code: "persisted_baseline_conflict" },
    code: "persisted_baseline_conflict",
    message:
      "O arquivo do Projeto foi alterado fora do MyAlbuns. O Salvamento não substituiu essas alterações.",
  },
  {
    wire: { code: "save_state_indeterminate" },
    code: "save_state_indeterminate",
    message:
      "Não foi possível confirmar qual revisão ficou no arquivo. Reabra o Projeto antes de continuar.",
  },
  {
    wire: { code: "recovery_cleanup_failed" },
    code: "recovery_cleanup_failed",
    message:
      "O arquivo do Projeto foi salvo, mas não foi possível encerrar a Recuperação. Tente salvar novamente.",
  },
  {
    wire: { code: "session_unavailable" },
    code: "session_unavailable",
    message:
      "A Sessão do Projeto não está mais disponível. Reabra o Projeto para continuar.",
  },
  {
    wire: { code: "not_found" },
    code: "not_found",
    message:
      "O arquivo do Projeto não foi encontrado. Confirme se ele foi movido ou removido.",
  },
  {
    wire: { code: "unavailable" },
    code: "unavailable",
    message:
      "O local do Projeto está indisponível. Reconecte a unidade ou o compartilhamento e tente novamente.",
  },
  {
    wire: { code: "access_denied" },
    code: "access_denied",
    message:
      "O Windows negou acesso ao arquivo do Projeto. Verifique as permissões e tente novamente.",
  },
  {
    wire: { code: "invalid_path" },
    code: "invalid_path",
    message: "O caminho do arquivo do Projeto não é válido.",
  },
  {
    wire: { code: "unexpected_object_type" },
    code: "unexpected_object_type",
    message: "O destino do Projeto deixou de ser um arquivo regular.",
  },
  {
    wire: { code: "conflict" },
    code: "conflict",
    message:
      "O arquivo do Projeto mudou durante o Salvamento. Tente novamente.",
  },
  {
    wire: { code: "io_failure" },
    code: "io_failure",
    message: "O Windows não conseguiu concluir o Salvamento do Projeto.",
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
    message: "A Imagem decorativa vinculada não está disponível.",
  });

  const failure = tauriMediaPreviewPort.prepareMediaPreviews({
    revision: 1,
    visibleMediaIds: ["media-a-001"],
    preloadMediaIds: [],
  });

  await expect(failure).rejects.toBeInstanceOf(MediaPreviewError);
  await expect(failure).rejects.toMatchObject({
    code: "unavailable",
    message: "A Imagem decorativa vinculada não está disponível.",
  });
});

test("normalizes typed unavailable-media retry failures at the IPC adapter", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({
    code: "read_failed",
    message: "A nova inspeção não pôde ser concluída.",
  });

  const failure = tauriMediaPreviewPort.retryUnavailableMedia("media-a-001");

  await expect(failure).rejects.toBeInstanceOf(MediaPreviewError);
  await expect(failure).rejects.toMatchObject({
    code: "read_failed",
    message: "A nova inspeção não pôde ser concluída.",
  });
});
