import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { createTauriProjectDialogPort } from "./tauriProjectDialogPort";
import { tauriProjectCorePort } from "./tauriProjectPorts";
import { useProjectMutations } from "../components/useProjectMutations";
import { useProjectMutationRunner } from "../components/useProjectMutationRunner";
import { useAlbumInformationApplyController } from "../components/useAlbumInformationApplyController";
import { createAlbumInformationProjectDraft } from "../application/projectSettingsDraft";
import { createAlbumInformationReview } from "../application/albumInformationReview";
import type { ProjectCorePort } from "../application/projectPorts";
import { createThreeSheetProjection } from "../test/projectFixtures";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn(), Channel: class {} }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));

test("conversion and Album information can share the owned dialog while Save is pending", async () => {
  let emitNative!: (payload: unknown) => void;
  vi.mocked(invoke).mockResolvedValue(undefined);
  vi.mocked(listen).mockImplementation(async (_event, handler) => {
    emitNative = (payload) => handler({ payload } as never);
    return () => undefined;
  });
  const initial = createThreeSheetProjection();
  initial.state.album.sheets[2].visuals = {
    background: { kind: "perSide", left: { kind: "default" }, right: {
      kind: "custom", content: { kind: "color", rgb: "#123456" }, mapping: "side",
    } },
    overlay: { kind: "default" },
  };
  initial.state.album.sheets[2].edgeConversionLoss = { sheetId: "sheet-003", sheetNumber: 3, side: "right",
    background: { kind: "custom", content: { kind: "color", rgb: "#123456" }, mapping: "side" }, overlay: null };
  let finishSave!: () => void;
  const pendingSave = new Promise<void>((resolve) => { finishSave = resolve; });
  const apply = vi.fn<ProjectCorePort["apply"]>(async () => initial);
  const impact = { conversionLosses: [], sheetWidthPx: 14_173, pageWidthPx: 7_087, heightPx: 7_087 };
  const port: ProjectCorePort = {
    ...tauriProjectCorePort,
    apply,
    applyWithOutcome: async (intent, progress) => ({
      projection: await apply(intent, progress), affectedFrameId: null, affectedSheetId: null,
    }),
    validateAlbumInformation: async () => ({ errors: [], impact }),
    save: async (revision) => {
      await pendingSave;
      return { outcome: { kind: "saved", revision }, projection: initial };
    },
  };
  const dialogPort = createTauriProjectDialogPort();
  const view = renderHook(() => {
    const mutations = useProjectMutations({
      projection: initial, projectDialogPort: dialogPort,
      runProjectMutation: useProjectMutationRunner(initial.state.projectId, port),
      onProjectionChange: vi.fn(), onAffectedFrame: vi.fn(), onAffectedSheet: vi.fn(),
    });
    const information = useAlbumInformationApplyController({
       projectDialogPort: dialogPort,
      onApply: mutations.applyAlbumInformation, onError: vi.fn(),
    });
    return { mutations, information };
  });
  const baseline = { ...initial.state.document, firstSheet: "double" as const, lastSheet: "double" as const };
  const draft = createAlbumInformationProjectDraft(initial.state.revision, baseline).transition({ ...baseline, dpi: 600 });
  let conversion!: Promise<boolean>;
  let information!: Promise<boolean>;
  act(() => {
    view.result.current.mutations.save();
    conversion = view.result.current.mutations.applyWithOutcome({ kind: "convertEdgeSheet", sheetId: "sheet-003" });
    information = view.result.current.information.requestApply(draft, impact);
  });
  const presentation = (kind: string) => vi.mocked(invoke).mock.calls
    .find(([command, args]) => command === "present_project_dialog" &&
      (args as { state: { kind: string } }).state.kind === kind)?.[1] as { sessionId: string } | undefined;
  await waitFor(() => expect(presentation("albumInformationConfirmation")).toBeDefined());
  await act(async () => {
    emitNative({ sessionId: presentation("albumInformationConfirmation")!.sessionId, action: "confirmAlbumInformation" });
    finishSave();
    expect(await information).toBe(true);
  });
  await waitFor(() => expect(presentation("edgeConversionConfirmation")).toBeDefined());
  expect(apply).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith(expect.objectContaining({ kind: "setAlbumInformation" }), expect.any(Function));
  await act(async () => {
    emitNative({ sessionId: presentation("edgeConversionConfirmation")!.sessionId, action: "confirmEdgeConversion" });
    expect(await conversion).toBe(true);
  });
  expect(apply).toHaveBeenCalledTimes(2);
  expect(apply).toHaveBeenLastCalledWith({ kind: "convertEdgeSheet", sheetId: "sheet-003" }, expect.any(Function));
});

test.each([false, true])("dimensional review follows queued Save and preserves its guard (Save failure: %s)", async (failSave) => {
  const initial = createThreeSheetProjection();
  const baseline = { ...initial.state.document, firstSheet: "double" as const, lastSheet: "double" as const };
  const value = { ...baseline, sheetWidthUm: baseline.sheetWidthUm * 1.05 };
  const draft = createAlbumInformationProjectDraft(initial.state.revision, baseline).transition(value);
  const impact = (key: string) => ({ conversionLosses: [], sheetWidthPx: 7_441, pageWidthPx: 3_720, heightPx: 3_543,
    dimensionalChange: { proportionChanged: true, confirmationKey: key } });
  let finishSave!: () => void;
  const pending = new Promise<void>((resolve) => { finishSave = resolve; });
  const apply = vi.fn<ProjectCorePort["apply"]>(async () => initial);
  const port: ProjectCorePort = { ...tauriProjectCorePort, apply,
    validateAlbumInformation: async () => ({ errors: [], impact: impact("new-source") }),
    save: async (revision) => {
      await pending;
      if (failSave) throw new Error("Save failed");
      return { outcome: { kind: "saved", revision }, projection: initial };
    },
  };
  const view = renderHook(() => useProjectMutations({ projection: initial,
    runProjectMutation: useProjectMutationRunner(initial.state.projectId, port),
    projectDialogPort: createTauriProjectDialogPort(), onProjectionChange: vi.fn(),
    onAffectedFrame: vi.fn(), onAffectedSheet: vi.fn(),
  }));
  let result!: ReturnType<typeof view.result.current.applyAlbumInformation>;
  act(() => {
    void view.result.current.save();
    result = view.result.current.applyAlbumInformation(draft,
      createAlbumInformationReview(baseline, value, impact("old-source")));
  });
  expect(apply).not.toHaveBeenCalled();
  await act(async () => { finishSave(); });
  const review = await result;
  expect(review.kind).toBe("reviewRequired");
  expect(apply).not.toHaveBeenCalled();
  if (review.kind !== "reviewRequired") throw new Error("Expected a new review");
  await act(async () => {
    expect(await view.result.current.applyAlbumInformation(draft, review.review)).toEqual({ kind: "completed" });
  });
  expect(apply).toHaveBeenCalledWith(expect.objectContaining({ expectedDimensionKey: "new-source" }), expect.any(Function));
});

test("a source observation racing the native commit reopens confirmation without retrying automatically", async () => {
  const initial = createThreeSheetProjection();
  const baseline = { ...initial.state.document, firstSheet: "double" as const, lastSheet: "double" as const };
  const value = { ...baseline, sheetWidthUm: baseline.sheetWidthUm * 1.05 };
  const draft = createAlbumInformationProjectDraft(initial.state.revision, baseline).transition(value);
  let key = "before";
  const impact = () => ({ conversionLosses: [], sheetWidthPx: 7_441, pageWidthPx: 3_720, heightPx: 3_543,
    dimensionalChange: { proportionChanged: true, confirmationKey: key } });
  const originalImpact = impact();
  const apply = vi.fn<ProjectCorePort["apply"]>(async () => {
    key = "after";
    throw new Error("The native dimensional guard rejected stale observations");
  });
  const port: ProjectCorePort = { ...tauriProjectCorePort, apply,
    validateAlbumInformation: async () => ({ errors: [], impact: impact() }) };
  const view = renderHook(() => useProjectMutations({ projection: initial,
    runProjectMutation: useProjectMutationRunner(initial.state.projectId, port),
    projectDialogPort: createTauriProjectDialogPort(), onProjectionChange: vi.fn(),
    onAffectedFrame: vi.fn(), onAffectedSheet: vi.fn(),
  }));
  await act(async () => {
    const result = await view.result.current.applyAlbumInformation(draft,
      createAlbumInformationReview(baseline, value, originalImpact));
    expect(result).toMatchObject({ kind: "reviewRequired", review: { impact: { dimensionalChange: { confirmationKey: "after" } } } });
  });
  expect(apply).toHaveBeenCalledOnce();
});
