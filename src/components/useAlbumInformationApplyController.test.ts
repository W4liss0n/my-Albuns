import { act, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
} from "../application/projectDialogPort";
import { createAlbumInformationProjectDraft } from "../application/projectSettingsDraft";
import type { AlbumInformation } from "../domain/project";
import {
  albumInformationDetails,
  useAlbumInformationApplyController,
} from "./useAlbumInformationApplyController";

const baseline: AlbumInformation = {
  displayUnit: "mm",
  sheetWidthUm: 600_000,
  sheetHeightUm: 300_000,
  dpi: 300,
  bleedUm: 3_000,
  safetyUm: 3_000,
  firstSheet: "double",
  lastSheet: "double",
};

test("describes only the Album information field that actually changed", () => {
  const details = albumInformationDetails(
    { ...baseline, firstSheet: "singlePage" },
    baseline,
    {
      sheetWidthPx: 7_087,
      pageWidthPx: 3_543,
      heightPx: 3_543,
    },
  );

  expect(details).toEqual([
    {
      label: "Primeira Lâmina",
      value: "Lâmina dupla → Página única",
    },
  ]);
});

test("describes final raster size and structural and dimensional impact", () => {
  const details = albumInformationDetails(
    {
      ...baseline,
      sheetWidthUm: 700_000,
      sheetHeightUm: 350_000,
      dpi: 240,
      firstSheet: "singlePage",
    },
    baseline,
    {
      sheetWidthPx: 6_614,
      pageWidthPx: 3_307,
      heightPx: 3_307,
    },
  );

  expect(details).toEqual([
    {
      label: "Primeira Lâmina",
      value: "Lâmina dupla → Página única",
    },
    { label: "DPI", value: "300 → 240" },
    { label: "Largura da Lâmina", value: "600 mm → 700 mm" },
    { label: "Altura da Lâmina", value: "300 mm → 350 mm" },
    {
      label: "Resolução resultante",
      value: "Lâmina 6.614 × 3.307 px · Página 3.307 × 3.307 px",
    },
    {
      label: "Composição",
      value: "A proporção será preservada no novo formato.",
    },
  ]);
});

test("uses the selected Unit for changed measurements without unrelated raster details", () => {
  const details = albumInformationDetails(
    {
      ...baseline,
      bleedUm: 5_000,
      displayUnit: "cm",
    },
    baseline,
    {
      sheetWidthPx: 7_087,
      pageWidthPx: 3_543,
      heightPx: 3_543,
    },
  );

  expect(details).toEqual([
    { label: "Unidade", value: "mm → cm" },
    { label: "Sangria", value: "0.3 cm → 0.5 cm" },
  ]);
});

function dialogHarness() {
  let listener: ((action: ProjectDialogAction) => void) | null = null;
  const dismiss = vi.fn(async () => undefined);
  const present = vi.fn(async () => undefined);
  const port: ProjectDialogPort = {
    acquire: (nextListener) => {
      listener = nextListener;
      return { dismiss, present };
    },
  };
  return {
    dismiss,
    emit(action: ProjectDialogAction) {
      listener?.(action);
    },
    port,
    present,
  };
}

const changedDraft = createAlbumInformationProjectDraft(3, baseline).transition({
  ...baseline,
  dpi: 600,
});
const impact = {
  heightPx: 7_087,
  pageWidthPx: 7_087,
  sheetWidthPx: 14_173,
};

test("completes the Apply request only after the owned confirmation commits", async () => {
  const dialog = dialogHarness();
  const onApply = vi.fn(async () => ({ kind: "completed" as const }));
  const { result } = renderHook(() =>
    useAlbumInformationApplyController({
      projectDialogPort: dialog.port,
      onApply,
      onError: vi.fn(),
    }),
  );

  let completion!: Promise<boolean>;
  await act(async () => {
    completion = result.current.requestApply(changedDraft, impact);
    await Promise.resolve();
  });
  expect(result.current.active).toBe(true);

  await act(async () => {
    dialog.emit("confirmAlbumInformation");
    await expect(completion).resolves.toBe(true);
  });

  expect(onApply).toHaveBeenCalledOnce();
  expect(result.current.active).toBe(false);
  expect(dialog.dismiss).toHaveBeenCalledOnce();
});

test("resolves cancellation and a rejected commit without orphaning command blocking", async () => {
  const dialog = dialogHarness();
  const onApply = vi.fn(async () => ({ kind: "rejected" as const }));
  const { result } = renderHook(() =>
    useAlbumInformationApplyController({
      projectDialogPort: dialog.port,
      onApply,
      onError: vi.fn(),
    }),
  );

  let cancelled!: Promise<boolean>;
  await act(async () => {
    cancelled = result.current.requestApply(changedDraft, impact);
    await Promise.resolve();
    dialog.emit("cancelAlbumInformation");
    await expect(cancelled).resolves.toBe(false);
  });
  expect(result.current.active).toBe(false);

  let rejected!: Promise<boolean>;
  await act(async () => {
    rejected = result.current.requestApply(changedDraft, impact);
    await Promise.resolve();
    dialog.emit("confirmAlbumInformation");
    await expect(rejected).resolves.toBe(false);
  });
  expect(result.current.active).toBe(false);
});

test("resolves false when confirmation presentation or the commit fails", async () => {
  const presentationFailure = new Error("Falha ao abrir a confirmação.");
  const dialog = dialogHarness();
  dialog.present.mockRejectedValueOnce(presentationFailure);
  const onError = vi.fn();
  const onApply = vi.fn(async () => {
    throw new Error("Falha ao aplicar a alteração.");
  });
  const { result } = renderHook(() =>
    useAlbumInformationApplyController({
      projectDialogPort: dialog.port,
      onApply,
      onError,
    }),
  );

  await act(async () => {
    await expect(result.current.requestApply(changedDraft, impact)).resolves.toBe(
      false,
    );
  });
  expect(onError).toHaveBeenCalledWith(presentationFailure.message);
  expect(result.current.active).toBe(false);

  let completion!: Promise<boolean>;
  await act(async () => {
    completion = result.current.requestApply(changedDraft, impact);
    await Promise.resolve();
    dialog.emit("confirmAlbumInformation");
    await expect(completion).resolves.toBe(false);
  });
  expect(onError).toHaveBeenLastCalledWith("Falha ao aplicar a alteração.");
  expect(result.current.active).toBe(false);
});

test("settles an outstanding Apply completion when its controller unmounts", async () => {
  const dialog = dialogHarness();
  const { result, unmount } = renderHook(() =>
    useAlbumInformationApplyController({
      projectDialogPort: dialog.port,
      onApply: vi.fn(async () => ({ kind: "completed" as const })),
      onError: vi.fn(),
    }),
  );

  let completion!: Promise<boolean>;
  await act(async () => {
    completion = result.current.requestApply(changedDraft, impact);
    await Promise.resolve();
  });
  unmount();

  await expect(completion).resolves.toBe(false);
  expect(dialog.dismiss).toHaveBeenCalledOnce();
});
