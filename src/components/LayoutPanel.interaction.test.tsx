import { useState } from "react";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { EditorProjection, LayoutQueryResult, LayoutSelection, ProjectIntent } from "../domain/project";
import { layoutPanelCorpus } from "../test/layoutPanelPreview";
import { LayoutPanel } from "./LayoutPanel";
import { useLayoutPanel } from "./useLayoutPanel";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

test("locking keeps the displayed thumbnails mounted while the refreshed query is pending", async () => {
  const sample = layoutPanelCorpus.cases.mixed;
  const before = sample.before;
  const locked = sample.locked!;
  const sheetId = before.projection.state.album.sheets[0].id;
  const prepared = before.queries[sheetId];
  const refreshed = { ...locked.queries[sheetId].query, queryId: "locked-refresh" };
  let resolveRefresh!: (query: LayoutQueryResult) => void;
  const refresh = new Promise<LayoutQueryResult>((resolve) => { resolveRefresh = resolve; });
  const queryLayouts = vi.fn().mockResolvedValueOnce(prepared.query).mockReturnValue(refresh);
  const previewLayout = vi.fn(async (selection: LayoutSelection) =>
    (selection.queryId === refreshed.queryId ? locked : before).queries[sheetId].previews[selection.candidateIndex]);
  let publishProjection!: (projection: EditorProjection) => void;
  const commit = vi.fn(async (intent: ProjectIntent) => {
    expect(intent.kind).toBe("lockLayout");
    publishProjection(locked.projection);
    return true;
  });
  const runner = { waitForIdle: async () => null } as unknown as ProjectMutationRunner;

  function Workspace() {
    const [projection, setProjection] = useState(before.projection);
    publishProjection = setProjection;
    const panel = useLayoutPanel({ projection, editing: false, disabled: false,
      port: { queryLayouts, previewLayout }, runner, commit, onError: vi.fn() });
    return <>
      <button aria-controls="layout-panel" onClick={() => panel.toggle(sheetId)}>Layouts da Lâmina</button>
      {panel.visible && <LayoutPanel controller={panel} sheet={projection.composition.sheets[0]} />}
    </>;
  }

  render(<Workspace />);
  fireEvent.click(screen.getByRole("button", { name: "Layouts da Lâmina" }));
  const lock = await screen.findByRole("button", { name: "Aplicar e travar Layout 1" });
  const panel = screen.getByRole("region", { name: "Painel de Layouts" });
  const thumbnail = within(panel).getByRole("button", { name: /^Aplicar Layout 1(?: — último aplicado)?$/ });
  fireEvent.click(lock);
  await waitFor(() => expect(queryLayouts).toHaveBeenCalledTimes(2));

  expect(thumbnail).toBeInTheDocument();
  expect(thumbnail).toBeDisabled();
  expect(within(panel).queryByText("Consultando Layouts…")).not.toBeInTheDocument();
  fireEvent.click(lock);
  expect(commit).toHaveBeenCalledOnce();

  await act(async () => { resolveRefresh(refreshed); });
  await waitFor(() => expect(screen.getByRole("button", { name: "Destravar Layout da Lâmina 01" })).toBeEnabled());
  expect(screen.getByRole("region", { name: "Painel de Layouts" })).toBe(panel);
});

test("outside presses close the panel, while its controls and Sheet bars keep their own actions", async () => {
  const sample = layoutPanelCorpus.cases.mixed.before;
  const sheets = sample.projection.state.album.sheets;
  let queriedSheetId = sheets[0].id;
  const queryLayouts = vi.fn(async (sheetId: string) => {
    queriedSheetId = sheetId;
    return sample.queries[sheetId].query;
  });
  const previewLayout = async (selection: LayoutSelection) => sample.queries[queriedSheetId].previews[selection.candidateIndex];
  const runner = { waitForIdle: async () => null } as unknown as ProjectMutationRunner;
  const outsideAction = vi.fn();

  function Workspace() {
    const panel = useLayoutPanel({ projection: sample.projection, editing: false, disabled: false,
      port: { queryLayouts, previewLayout }, runner, commit: async () => true, onError: vi.fn() });
    return <>
      {sheets.map((sheet) => <button key={sheet.id} aria-controls="layout-panel"
        onPointerDown={(event) => event.stopPropagation()} onClick={() => panel.toggle(sheet.id)}>
        Layouts da Lâmina {sheet.number}
      </button>)}
      <button onPointerDown={(event) => event.stopPropagation()} onClick={outsideAction}>Outra ação</button>
      {panel.visible && <LayoutPanel controller={panel}
        sheet={sample.projection.composition.sheets.find((sheet) => sheet.sheetId === panel.sheetId)!} />}
    </>;
  }
  const click = (element: HTMLElement) => { fireEvent.pointerDown(element); fireEvent.click(element); };
  render(<Workspace />);
  const firstBar = screen.getByRole("button", { name: "Layouts da Lâmina 1" });
  const secondBar = screen.getByRole("button", { name: "Layouts da Lâmina 2" });
  click(firstBar);
  await screen.findByRole("button", { name: "Aplicar e travar Layout 1" });
  const panel = screen.getByRole("region", { name: "Painel de Layouts" });
  click(within(panel).getByRole("combobox", { name: "Quantidade de Frames" }));
  expect(panel).toBeInTheDocument();
  click(secondBar);
  await waitFor(() => expect(queryLayouts).toHaveBeenLastCalledWith(sheets[1].id));
  expect(panel).toBeInTheDocument();
  click(secondBar);
  expect(panel).not.toBeInTheDocument();
  click(firstBar);
  await screen.findByRole("button", { name: "Aplicar e travar Layout 1" });
  click(screen.getByRole("button", { name: "Outra ação" }));
  expect(screen.queryByRole("region", { name: "Painel de Layouts" })).not.toBeInTheDocument();
  expect(outsideAction).toHaveBeenCalledOnce();
});
