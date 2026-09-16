import { createRef, useState } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import type { MediaCatalogItem, MediaFolder, MediaFolderEdit } from "../domain/project";
import { MediaPanel, type MediaPanelHandle } from "./MediaPanel";

const items: MediaCatalogItem[] = [
  { id: "p1", kind: "photo", name: "001.jpg", sourceWidthPx: 600, sourceHeightPx: 400, palette: null },
  { id: "p2", kind: "photo", name: "002.jpg", sourceWidthPx: 600, sourceHeightPx: 400, palette: null },
  { id: "d1", kind: "decorative", name: "Fundo.jpg", sourceWidthPx: 600, sourceHeightPx: 400, palette: null },
];
const folders: MediaFolder[] = [
  { id: "a", kind: "photo", name: "Turma A", mediaIds: ["p1"] },
  { id: "b", kind: "photo", name: "Turma B", mediaIds: ["p2"] },
  { id: "c", kind: "decorative", name: "Fundos", mediaIds: ["d1"] },
];
function harness(initial = folders) {
  const edit = vi.fn<(edit: MediaFolderEdit) => Promise<boolean>>(async () => true);
  const remove = vi.fn();
  const ref = createRef<MediaPanelHandle>();
  const props = { ref, mediaItems: items, mediaUsage: [], mediaFolders: initial, onEditMediaFolder: edit,
    onFillPhoto: vi.fn(), onApplyDecorative: vi.fn(), onImportMedia: vi.fn(), onRemoveMedia: remove,
    onMediaDragChange: vi.fn(), onRelinkMedia: vi.fn(), onReplaceMedia: vi.fn(), onRetryUnavailableMedia: async () => {},
    previewSource: { kind: "static" as const }, preferences: { kind: "local" as const } };
  const view = render(<MediaPanel {...props} />);
  return { view, edit, remove, ref, props };
}
function gridItems() { return [...document.querySelectorAll<HTMLElement>("[data-media-id]")].map((item) => item.dataset.mediaId); }

test("folders filter each tab independently and intersect search and absence", async () => {
  const h = harness();
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: /Pasta Turma A/ }));
  expect(gridItems()).toEqual(["p1"]);
  await user.type(screen.getByRole("searchbox"), "002");
  expect(gridItems()).toEqual([]);
  await user.clear(screen.getByRole("searchbox"));
  h.view.rerender(<MediaPanel {...h.props} mediaFiles={{ p1: { mediaId: "p1", state: "absent", createdAtMs: null, modifiedAtMs: null } }} />);
  await user.click(screen.getByRole("button", { name: /Ausentes/ }));
  expect(gridItems()).toEqual(["p1"]);
  await user.click(screen.getByRole("button", { name: "Decorativos" }));
  expect(screen.queryByRole("button", { name: /Pasta Turma A/ })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: /Pasta Fundos/ }));
  expect(gridItems()).toEqual(["d1"]);
  await user.click(screen.getByRole("button", { name: "Fotos" }));
  expect(gridItems()).toEqual(["p1"]);
  await user.click(screen.getByRole("button", { name: "Todas 2" }));
  expect(gridItems()).toEqual(["p1", "p2"]);
  expect(h.edit).not.toHaveBeenCalled();
});

test("create and rename use compact forms; invalid names never submit", async () => {
  const h = harness(); const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Nova pasta de organização" }));
  const dialog = screen.getByRole("dialog", { name: "Nova pasta" });
  const input = within(dialog).getByRole("textbox", { name: "Nome" });
  expect(input).not.toHaveAttribute("aria-invalid");
  await user.click(within(dialog).getByRole("button", { name: "Criar" }));
  expect(input).toHaveAttribute("aria-invalid", "true");
  expect(h.edit).not.toHaveBeenCalled();
  await user.type(input, "turma a");
  await user.click(within(dialog).getByRole("button", { name: "Criar" }));
  expect(h.edit).not.toHaveBeenCalled();
  await user.clear(input); await user.type(input, "Retratos");
  await user.keyboard("{Enter}");
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "create", mediaKind: "photo", name: "Retratos" });
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  fireEvent.contextMenu(screen.getByRole("button", { name: /Pasta Turma A/ }));
  await user.click(screen.getByRole("menuitem", { name: "Renomear…" }));
  const rename = screen.getByRole("textbox", { name: "Nome" });
  await user.clear(rename); await user.type(rename, "Cerimônia{Enter}");
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "rename", folderId: "a", name: "Cerimônia" });
});

test("move acts on selected images and can remove the folder association", async () => {
  const h = harness(); const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "001.jpg" }));
  fireEvent.click(screen.getByRole("button", { name: "002.jpg" }), { ctrlKey: true });
  fireEvent.contextMenu(screen.getByRole("button", { name: "001.jpg" }));
  await user.click(screen.getByRole("menuitem", { name: "Mover para pasta…" }));
  await user.selectOptions(screen.getByRole("combobox", { name: "Destino" }), "b");
  await user.click(screen.getByRole("button", { name: "Mover" }));
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "moveMedia", mediaIds: ["p1", "p2"], folderId: "b" });
  fireEvent.contextMenu(screen.getByRole("button", { name: "001.jpg" }));
  await user.click(screen.getByRole("menuitem", { name: "Mover para pasta…" }));
  await user.selectOptions(screen.getByRole("combobox", { name: "Destino" }), "");
  await user.click(screen.getByRole("button", { name: "Mover" }));
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "moveMedia", mediaIds: ["p1", "p2"], folderId: null });
  expect(h.remove).not.toHaveBeenCalled();
});

test("deleting an active folder returns to All; undo does not revive the stale filter", async () => {
  const h = harness(); const user = userEvent.setup();
  const chip = screen.getByRole("button", { name: /Pasta Turma A/ });
  await user.click(chip); fireEvent.contextMenu(chip);
  await user.click(screen.getByRole("menuitem", { name: "Excluir pasta" }));
  expect(h.edit).toHaveBeenCalledWith({ kind: "delete", folderId: "a" });
  h.view.rerender(<MediaPanel {...h.props} mediaFolders={folders.slice(1)} />);
  expect(gridItems()).toEqual(["p1", "p2"]);
  h.view.rerender(<MediaPanel {...h.props} />);
  expect(screen.getByRole("button", { name: "Todas 2" })).toHaveAttribute("aria-pressed", "true");
  expect(h.remove).not.toHaveBeenCalled();
});

test("folder change discards hidden selection and catalog planning uses new membership", async () => {
  const h = harness(); const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "002.jpg" }));
  await user.click(screen.getByRole("button", { name: /Pasta Turma A/ }));
  fireEvent.keyDown(screen.getByRole("region", { name: "Painel de imagens" }), { key: "Delete" });
  expect(h.remove).toHaveBeenLastCalledWith([]);
  const plan = h.ref.current!.planCatalog(items, [], [{ ...folders[0], mediaIds: ["p2"] }]);
  expect(plan.demand.visibleMediaIds).not.toContain("p1");
});

test("Escape and Cancel close without edits; pending submission is not duplicated", async () => {
  const h = harness(); const user = userEvent.setup();
  const trigger = screen.getByRole("button", { name: "Nova pasta de organização" });
  await user.click(trigger); await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  await user.click(trigger); await user.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(h.edit).not.toHaveBeenCalled();
  let finish!: (result: boolean) => void;
  h.edit.mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  await user.click(trigger); await user.type(screen.getByRole("textbox"), "Retratos");
  await user.dblClick(screen.getByRole("button", { name: "Criar" }));
  expect(h.edit).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("button", { name: "Cancelar" })).toBeDisabled();
  finish(true);
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
});


test("losing an active folder clears its Ausentes filter and returns to All", async () => {
  const h = harness(); const user = userEvent.setup();
  h.view.rerender(<MediaPanel {...h.props} mediaFiles={{ p1: { mediaId: "p1", state: "absent", createdAtMs: null, modifiedAtMs: null } }} />);
  await user.click(screen.getByRole("button", { name: /Pasta Turma A/ }));
  await user.click(screen.getByRole("button", { name: /Ausentes/ }));
  expect(gridItems()).toEqual(["p1"]);
  h.view.rerender(<MediaPanel {...h.props} mediaFolders={folders.slice(1)}
    mediaFiles={{ p1: { mediaId: "p1", state: "absent", createdAtMs: null, modifiedAtMs: null } }} />);
  expect(screen.getByRole("button", { name: "Todas 2" })).toHaveAttribute("aria-pressed", "true");
  expect(gridItems()).toEqual(["p1", "p2"]);
});

test("moving the focused thumbnail out of the active folder restores connected panel focus", async () => {
  const h = harness(); h.view.unmount(); const user = userEvent.setup();
  function UpdatingPanel() {
    const [current, setCurrent] = useState(folders);
    return <MediaPanel {...h.props} mediaFolders={current} onEditMediaFolder={async () => {
      await Promise.resolve();
      setCurrent([{ ...folders[0], mediaIds: [] }, { ...folders[1], mediaIds: ["p1", "p2"] }, folders[2]]);
      return true;
    }} />;
  }
  render(<UpdatingPanel />);
  await user.click(screen.getByRole("button", { name: /Pasta Turma A/ }));
  fireEvent.contextMenu(screen.getByRole("button", { name: "001.jpg" }));
  await user.click(screen.getByRole("menuitem", { name: "Mover para pasta…" }));
  await user.selectOptions(screen.getByRole("combobox", { name: "Destino" }), "b");
  await user.click(screen.getByRole("button", { name: "Mover" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(gridItems()).toEqual([]);
  await waitFor(() => expect(screen.getByRole("region", { name: "Painel de imagens" })).toHaveFocus());
});
