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
  let finishSave!: () => void;
  const pendingSave = new Promise<void>((resolve) => { finishSave = resolve; });
  const apply = vi.fn<ProjectCorePort["apply"]>(async () => initial);
  const impact = { sheetWidthPx: 14_173, pageWidthPx: 7_087, heightPx: 7_087 };
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
      sheets: initial.state.album.sheets, projectDialogPort: dialogPort,
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
