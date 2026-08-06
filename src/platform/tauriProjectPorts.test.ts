import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";

import type { ProjectIntent } from "../domain/project";
import {
  MediaPreviewError,
  SaveProjectError,
} from "../application/projectPorts";
import { representativeProjection } from "../test/projectFixtures";
import {
  tauriExportPort,
  tauriMediaPreviewPort,
  tauriProjectSessionPort,
} from "./tauriProjectPorts";

const tauriBoundary = vi.hoisted(() => ({
  channels: [] as Array<{ onmessage: (message: unknown) => void }>,
}));

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

beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockResolvedValue(undefined);
  tauriBoundary.channels.length = 0;
});

test("completes an Export attempt with the backend result", async () => {
  const result = {
    widthPx: 7_087,
    heightPx: 3_543,
  };
  vi.mocked(invoke).mockResolvedValueOnce(result);

  const attempt = tauriExportPort.startSheet("sheet-001", vi.fn());

  await expect(attempt.completion).resolves.toEqual({
    status: "completed",
    result,
  });
  expect(invoke).toHaveBeenCalledWith("export_sheet", {
    sheetId: "sheet-001",
    onEvent: tauriBoundary.channels[0],
  });
});

test("forwards Export events without exposing the backend operation id", () => {
  const onEvent = vi.fn();

  tauriExportPort.startSheet("sheet-001", onEvent);
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

  const attempt = tauriExportPort.startSheet("sheet-001", vi.fn());
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

  const attempt = tauriExportPort.startSheet("sheet-001", vi.fn());

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

  const attempt = tauriExportPort.startSheet("sheet-001", vi.fn());
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

  const attempt = tauriExportPort.startSheet("sheet-001", vi.fn());
  const cancellation = attempt.cancel();

  await expect(attempt.completion).rejects.toBe(failure);
  await expect(cancellation).resolves.toBe("not_found");
  expect(invoke).toHaveBeenCalledTimes(1);
});

test("maps the Project and media ports to the desktop commands", async () => {
  const intent: ProjectIntent = {
    kind: "fillLeftmostPlaceholder",
    sheetId: "sheet-002",
    mediaId: "media-campo",
  };

  await tauriProjectSessionPort.load("project-load-1");
  await tauriProjectSessionPort.apply(intent);
  await tauriProjectSessionPort.undo();
  await tauriProjectSessionPort.redo();
  vi.mocked(invoke).mockResolvedValueOnce([
    {
      mediaId: "media-a-001",
      url: "http://asset.localhost/cache-preview",
    },
  ]);
  const previews = await tauriMediaPreviewPort.prepareMediaPreviews();

  expect(invoke).toHaveBeenNthCalledWith(1, "project_state", {
    operationId: "project-load-1",
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "apply_project_intent", {
    intent,
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "undo_project");
  expect(invoke).toHaveBeenNthCalledWith(4, "redo_project");
  expect(invoke).toHaveBeenNthCalledWith(5, "prepare_media_previews");
  expect(previews?.[0].url).toBe("http://asset.localhost/cache-preview");
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

  await expect(tauriProjectSessionPort.save(25)).resolves.toEqual(
    result,
  );
  expect(invoke).toHaveBeenCalledWith("save_project", {
    expectedRevision: 25,
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

  await expect(tauriProjectSessionPort.save(25)).resolves.toEqual(
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

  await expect(tauriProjectSessionPort.save(25)).rejects.toMatchObject({
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

  await expect(tauriProjectSessionPort.save(25)).rejects.toMatchObject({
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

    const failure = tauriProjectSessionPort.save(25);

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

  const failure = tauriMediaPreviewPort.prepareMediaPreviews();

  await expect(failure).rejects.toBeInstanceOf(MediaPreviewError);
  await expect(failure).rejects.toMatchObject({
    code: "unavailable",
    message: "A Imagem decorativa vinculada não está disponível.",
  });
});
