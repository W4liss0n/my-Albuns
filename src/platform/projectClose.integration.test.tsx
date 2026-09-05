import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useState } from "react";
import { expect, test, vi } from "vitest";

import type { ProjectCorePort } from "../application/projectPorts";
import type { LogEvent } from "../application/logging";
import type { EditorProjection } from "../domain/project";
import { representativeProjection as projection } from "../test/projectFixtures";
import { ProjectWorkspace } from "../components/ProjectWorkspace";
import { useProjectMutationRunner } from "../components/useProjectMutationRunner";
import { LoggingProvider } from "../components/loggingContext";
import { tauriProjectWindowPort } from "./tauriProjectWindowPort";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("../components/AlbumCanvas", () => ({ AlbumCanvas: () => <div /> }));

function deferredProjection() {
  let resolve!: (projection: EditorProjection) => void;
  const promise = new Promise<EditorProjection>((complete) => { resolve = complete; });
  return { promise, resolve };
}

function projectWindowHarness() {
  const present = vi.fn(async () => undefined);
  return { dialog: { present, port: { acquire: () => ({
    present, dismiss: async () => undefined,
  }) } } };
}

function getApplicationCommand(menuName: string, commandName: string) {
  fireEvent.click(screen.getByRole("menuitem", { name: menuName }));
  return screen.getByRole("menuitem", { name: commandName });
}

function projectCorePortWithApply(
  apply: ProjectCorePort["apply"],
): ProjectCorePort {
  return {
    load: async () => projection,
    validateAlbumInformation: async () => ({
      errors: [],
      impact: { sheetWidthPx: 7_087, pageWidthPx: 3_543, heightPx: 3_543 },
    }),
    apply,
    applyWithOutcome: async (intent) => ({
      projection: await apply(intent),
      affectedFrameId: "frame-001",
      affectedSheetId: null,
    }),
    importPhoto: async () => ({ kind: "cancelled", projection }),
    resolvePhotoDropTarget: async () => ({ kind: "invalid" }),
    relink: async () => projection,
    undo: async () => projection,
    redo: async () => projection,
    save: async () => {
      throw new Error("Salvamento não configurado neste teste.");
    },
    saveAs: async () => {
      throw new Error("Salvar como não configurado neste teste.");
    },
  };
}

test.each(["pending", "completed"] as const)(
  "dispatches close to IPC after an applied change and a %s save",
  async (savePhase) => {
    const appliedProjection: EditorProjection = {
      ...projection,
      state: { ...projection.state, dirty: true, revision: projection.state.revision + 1 },
    };
    const savedProjection: EditorProjection = {
      ...appliedProjection,
      state: {
        ...appliedProjection.state,
        dirty: false,
        savedRevision: appliedProjection.state.revision,
      },
    };
    const pendingSave = deferredProjection();
    const corePort = projectCorePortWithApply(async () => appliedProjection);
    corePort.save = vi.fn(async () => ({
      outcome: { kind: "saved" as const, revision: appliedProjection.state.revision },
      projection: await pendingSave.promise,
    }));
    const close = projectWindowHarness();
    vi.mocked(listen).mockResolvedValue(() => undefined);
    vi.mocked(invoke).mockReset().mockResolvedValue({ kind: "closed" });

    function StatefulWorkspace() {
      const [current, setCurrent] = useState(projection);
      const runProjectMutation = useProjectMutationRunner(current.state.projectId, corePort);
      return (
        <ProjectWorkspace
          exportPipelinePort={{ startSheet: () => { throw new Error("No export in this scenario"); } }}
          runProjectMutation={runProjectMutation}
          mediaPreviews={{}}
          onMediaDemandChange={() => undefined}
          onRetryUnavailableMedia={async () => undefined}
          onGraphicsUnavailable={() => undefined}
          onPreferencesReady={() => undefined}
          workspacePreferences={{ kind: "memory" }}
          projection={current}
          projectCorePort={corePort}
          projectDialogPort={close.dialog.port}
          projectWindowPort={tauriProjectWindowPort}
          onProjectionChange={setCurrent}
        />
      );
    }

    const events: LogEvent[] = [];
    render(
      <LoggingProvider logger={{ write: (event) => events.push(event) }}>
        <StatefulWorkspace />
      </LoggingProvider>,
    );
    const albumDesign = within(
      screen.getByRole("button", { name: "Design do Álbum" }).closest("section") as HTMLElement,
    );
    fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
      target: { value: "#f7f5f0" },
    });
    fireEvent.click(albumDesign.getByRole("button", { name: "Aplicar" }));
    fireEvent.keyDown(window, { ctrlKey: true, key: "s" });
    await waitFor(() => expect(corePort.save).toHaveBeenCalledWith(appliedProjection.state.revision));

    if (savePhase === "pending") {
      fireEvent.click(getApplicationCommand("Arquivo", "Fechar Projeto"));
      expect(invoke).not.toHaveBeenCalled();
      expect(events.map(({ event }) => event)).toEqual(["project_close_requested"]);
    }
    await act(async () => {
      pendingSave.resolve(savedProjection);
      await pendingSave.promise;
    });
    if (savePhase === "completed") {
      fireEvent.click(getApplicationCommand("Arquivo", "Fechar Projeto"));
    }

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledExactlyOnceWith("request_project_close"),
    );
    expect(close.dialog.present).not.toHaveBeenCalled();
    expect(events.map(({ event }) => event)).toEqual([
      "project_close_requested",
      "project_close_mutations_settled",
      "project_close_ipc_requested",
      "project_close_ipc_resolved",
    ]);
    expect(new Set(events.map(({ operationId }) => operationId)).size).toBe(1);
    expect(events[0].operationId).toMatch(/^project-close-/);
  },
);
