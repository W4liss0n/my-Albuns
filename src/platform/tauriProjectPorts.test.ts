import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";

import type { ProjectIntent } from "../domain/project";
import { MediaPreviewError } from "../application/projectPorts";
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
    outputPath: "C:/exports/preview.pdf",
    widthPx: 7_087,
    heightPx: 3_543,
  };
  vi.mocked(invoke).mockResolvedValueOnce(result);

  const attempt = tauriExportPort.startPreview(vi.fn());

  await expect(attempt.completion).resolves.toEqual({
    status: "completed",
    result,
  });
  expect(invoke).toHaveBeenCalledWith("export_preview", {
    onEvent: tauriBoundary.channels[0],
  });
});

test("forwards Export events without exposing the backend operation id", () => {
  const onEvent = vi.fn();

  tauriExportPort.startPreview(onEvent);
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

  const attempt = tauriExportPort.startPreview(vi.fn());
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

  const attempt = tauriExportPort.startPreview(vi.fn());

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

  const attempt = tauriExportPort.startPreview(vi.fn());
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
    outputPath: "C:/exports/preview.pdf",
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

  const attempt = tauriExportPort.startPreview(vi.fn());
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
