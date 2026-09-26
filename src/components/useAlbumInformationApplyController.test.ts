import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
} from "../application/projectDialogPort";
import { createAlbumInformationProjectDraft } from "../application/projectSettingsDraft";
import { createAlbumInformationReview } from "../application/albumInformationReview";
import type { AlbumInformation, AlbumInformationImpact, EdgeConversionLoss } from "../domain/project";
import {
  albumInformationConsequences,
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

const noImpact: AlbumInformationImpact = { conversionLosses: [], sheetWidthPx: 7_087, pageWidthPx: 3_543, heightPx: 3_543 };
const backgroundLoss = (sheetNumber: number, side: "left" | "right"): EdgeConversionLoss => ({
  sheetId: `sheet-${sheetNumber}`, sheetNumber, side, overlay: null,
  background: { kind: "custom", content: { kind: "color", rgb: "#123456" }, mapping: "side" },
});
const overlayLoss = (sheetNumber: number, side: "left" | "right"): EdgeConversionLoss => ({
  ...backgroundLoss(sheetNumber, side), background: null,
  overlay: { kind: "custom", content: { kind: "media", mediaId: "overlay-001" }, mapping: "side" },
});
const consequencesOf = (information: AlbumInformation, impact: AlbumInformationImpact) =>
  albumInformationConsequences(createAlbumInformationReview(baseline, information, impact));

test("changes the panel already shows need no confirmation", () => {
  expect(consequencesOf({ ...baseline, dpi: 240, bleedUm: 5_000, safetyUm: 4_000, displayUnit: "cm" }, noImpact)).toEqual([]);
  expect(consequencesOf({ ...baseline, firstSheet: "singlePage" }, noImpact)).toEqual([]);
  expect(consequencesOf({ ...baseline, sheetWidthUm: 700_000, sheetHeightUm: 350_000 }, { ...noImpact,
    dimensionalChange: { proportionChanged: false, confirmationKey: "same-proportion" } })).toEqual([]);
});

test("warns about crops only when the Core reports a proportion change", () => {
  expect(consequencesOf({ ...baseline, sheetWidthUm: 630_000 }, { ...noImpact,
    dimensionalChange: { proportionChanged: true, confirmationKey: "review-a" } })).toEqual([
    "A proporção das lâminas muda. As fotos mantêm a proporção, e o recorte pode ser ajustado.",
  ]);
});

test("names the converted ends and every removed customization in one sentence", () => {
  expect(consequencesOf({ ...baseline, firstSheet: "singlePage" },
    { ...noImpact, conversionLosses: [backgroundLoss(1, "left")] })).toEqual([
    "A primeira lâmina vira página única. O fundo da lâmina 1 será removido.",
  ]);
  expect(consequencesOf({ ...baseline, lastSheet: "singlePage" },
    { ...noImpact, conversionLosses: [overlayLoss(18, "right")] })).toEqual([
    "A última lâmina vira página única. A sobreposição da lâmina 18 será removida.",
  ]);
  expect(consequencesOf({ ...baseline, firstSheet: "singlePage", lastSheet: "singlePage", sheetWidthUm: 630_000 }, {
    ...noImpact, conversionLosses: [backgroundLoss(1, "left"), overlayLoss(18, "right")],
    dimensionalChange: { proportionChanged: true, confirmationKey: "review-b" },
  })).toEqual([
    "A primeira e a última lâmina viram página única. O fundo da lâmina 1 e a sobreposição da lâmina 18 serão removidos.",
    "A proporção das lâminas muda. As fotos mantêm a proporção, e o recorte pode ser ajustado.",
  ]);
  expect(consequencesOf({ ...baseline, firstSheet: "singlePage" }, { ...noImpact, conversionLosses: [{
    ...backgroundLoss(1, "left"), overlay: overlayLoss(1, "left").overlay,
  }] })).toEqual([
    "A primeira lâmina vira página única. O fundo e a sobreposição da lâmina 1 serão removidos.",
  ]);
});

function dialogHarness(
  dismiss: () => Promise<void> = vi.fn(async () => undefined),
) {
  let listener: ((action: ProjectDialogAction) => void) | null = null;
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
  sheetWidthUm: 630_000,
});
const impact: AlbumInformationImpact = { conversionLosses: [],
  heightPx: 3_543,
  pageWidthPx: 3_720,
  sheetWidthPx: 7_441,
  dimensionalChange: { proportionChanged: true, confirmationKey: "review-a" },
};

test("applies directly, without a dialog, when nothing is removed or recropped", async () => {
  const dialog = dialogHarness();
  const onApply = vi.fn(async () => ({ kind: "completed" as const }));
  const { result } = renderHook(() =>
    useAlbumInformationApplyController({ projectDialogPort: dialog.port, onApply, onError: vi.fn() }),
  );
  const draft = createAlbumInformationProjectDraft(3, baseline).transition({ ...baseline, dpi: 600 });
  await act(async () => {
    await expect(result.current.requestApply(draft, noImpact)).resolves.toBe(true);
  });
  expect(dialog.present).not.toHaveBeenCalled();
  expect(onApply).toHaveBeenCalledOnce();
  expect(result.current.active).toBe(false);
});

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

test("keeps commands blocked until the owned confirmation releases its window", async () => {
  let releaseDialog!: () => void;
  const dismiss = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        releaseDialog = resolve;
      }),
  );
  const dialog = dialogHarness(dismiss);
  const { result } = renderHook(() =>
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
    dialog.emit("confirmAlbumInformation");
  });
  await waitFor(() => expect(dismiss).toHaveBeenCalledOnce());

  expect(result.current.active).toBe(true);
  let completed = false;
  void completion.then(() => {
    completed = true;
  });
  await Promise.resolve();
  expect(completed).toBe(false);

  await act(async () => {
    releaseDialog();
    await expect(completion).resolves.toBe(true);
  });
  expect(result.current.active).toBe(false);
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

test("aborts before committing when the owned busy projection fails", async () => {
  const busyFailure = new Error("Falha ao bloquear a confirmação.");
  const dialog = dialogHarness();
  dialog.present
    .mockResolvedValueOnce(undefined)
    .mockRejectedValueOnce(busyFailure);
  const onApply = vi.fn(async () => ({ kind: "completed" as const }));
  const onError = vi.fn();
  const { result } = renderHook(() =>
    useAlbumInformationApplyController({
      projectDialogPort: dialog.port,
      onApply,
      onError,
    }),
  );

  let completion!: Promise<boolean>;
  await act(async () => {
    completion = result.current.requestApply(changedDraft, impact);
    await Promise.resolve();
    dialog.emit("confirmAlbumInformation");
    await expect(completion).resolves.toBe(false);
  });

  expect(onApply).not.toHaveBeenCalled();
  expect(onError).toHaveBeenCalledWith(busyFailure.message);
  expect(dialog.dismiss).toHaveBeenCalledOnce();
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

test("asks before an edge conversion removes a customization", async () => {
  const dialog = dialogHarness();
  const onApply = vi.fn(async () => ({ kind: "completed" as const }));
  const view = renderHook(() => useAlbumInformationApplyController({ projectDialogPort: dialog.port, onApply, onError: vi.fn() }));
  const draft = createAlbumInformationProjectDraft(3, baseline).transition({ ...baseline, firstSheet: "singlePage" });
  let completion!: Promise<boolean>;
  await act(async () => { completion = view.result.current.requestApply(draft, { ...noImpact, conversionLosses: [{
    sheetId: "sheet-001", sheetNumber: 1, side: "left", overlay: null,
    background: { kind: "custom", content: { kind: "color", rgb: "#123456" }, mapping: "side" },
  }] }); });
  expect(dialog.present).toHaveBeenCalledOnce();
  expect(dialog.present).toHaveBeenCalledWith({ kind: "albumInformationConfirmation", busy: false, consequences: [
    "A primeira lâmina vira página única. O fundo da lâmina 1 será removido.",
  ] });
  await act(async () => { dialog.emit("cancelAlbumInformation"); expect(await completion).toBe(false); });
  expect(onApply).not.toHaveBeenCalled();
});
