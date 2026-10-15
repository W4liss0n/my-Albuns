import { createRef, useState } from "react";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, test, vi } from "vitest";
import type { MediaCatalogItem, MediaFolder, MediaFolderEdit, MediaFolderNameRequest, MediaFolderNameValidation } from "../../domain/project";
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
  const validate = vi.fn(async (request: MediaFolderNameRequest): Promise<MediaFolderNameValidation> => ({
    name: request.name, error: request.name === "" ? "empty" : request.name === "turma a" ? "nameInUse" : null,
  }));
  const remove = vi.fn();
  const ref = createRef<MediaPanelHandle>();
  const props = { ref, mediaItems: items, mediaUsage: [], mediaFolders: initial, onEditMediaFolder: edit, onValidateMediaFolderName: validate,
    onFillPhoto: vi.fn(), onApplyDecorative: vi.fn(), onImportMedia: vi.fn(), onRemoveMedia: remove,
    onMediaDragChange: vi.fn(), onRelinkMedia: vi.fn(), onReplaceMedia: vi.fn(), onRetryUnavailableMedia: async () => {},
    previewSource: { kind: "static" as const }, preferences: { kind: "local" as const } };
  const view = render(<MediaPanel {...props} />);
  return { view, edit, validate, remove, ref, props };
}
function gridItems() { return [...document.querySelectorAll<HTMLElement>("[data-media-id]")].map((item) => item.dataset.mediaId); }

const elementFromPointDescriptor = Object.getOwnPropertyDescriptor(document, "elementFromPoint");
afterEach(() => {
  if (elementFromPointDescriptor) Object.defineProperty(document, "elementFromPoint", elementFromPointDescriptor);
  else Reflect.deleteProperty(document, "elementFromPoint");
});
function dragHit(target: Element | null) {
  const hit = vi.fn(() => target);
  Object.defineProperty(document, "elementFromPoint", { configurable: true, value: hit });
  return hit;
}
function startDrag(name: string | RegExp = "001.jpg") {
  fireEvent.pointerDown(screen.getByRole("button", { name }), { pointerId: 7, button: 0, clientX: 20, clientY: 100 });
}
function moveDrag() { fireEvent.pointerMove(document, { pointerId: 7, clientX: 320, clientY: 16 }); }
function dropDrag() { fireEvent.pointerUp(document, { pointerId: 7, button: 0, clientX: 320, clientY: 16 }); }
function dragGhost() { return document.querySelector<HTMLElement>("[data-media-drag-ghost]"); }

test("the ghost reuses the thumbnail, follows the pointer beyond the panel, and disappears on drop", () => {
  const h = harness();
  h.view.rerender(<MediaPanel {...h.props} previewSource={{ kind: "static", previews: {
    p1: { mediaId: "p1", state: "ready", url: "http://myalbuns-cache.localhost/photo.jpg" },
  } }} />);
  dragHit(null); startDrag();
  expect(dragGhost()).toBeNull();
  fireEvent.pointerMove(document, { pointerId: 7, clientX: 22, clientY: 101 });
  expect(dragGhost()).toBeNull();
  moveDrag();
  const ghost = dragGhost()!;
  expect(ghost.parentElement).toBe(document.body);
  expect(ghost).toHaveAttribute("aria-hidden", "true");
  expect(ghost).toHaveAttribute("data-media-drag-ghost", "p1");
  expect(ghost.querySelector("img")).toHaveAttribute("src", "http://myalbuns-cache.localhost/photo.jpg");
  expect(ghost).toHaveStyle({ transform: "translate3d(326px, 22px, 0)" });
  expect(parseFloat(ghost.style.width)).toBe(80);
  expect(parseFloat(ghost.style.height)).toBeCloseTo(80 / 1.5);
  fireEvent.pointerMove(document, { pointerId: 7, clientX: 450, clientY: 250 });
  expect(dragGhost()).toBe(ghost);
  expect(ghost).toHaveStyle({ transform: "translate3d(456px, 256px, 0)" });
  dropDrag();
  expect(dragGhost()).toBeNull();
  expect(h.props.onMediaDragChange).toHaveBeenLastCalledWith(expect.objectContaining({ phase: "drop" }));
});

test("over a Sheet the Decorative ghost becomes one card that names where it will be applied", () => {
  const h = harness();
  fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));
  dragHit(null); startDrag("Fundo.jpg"); moveDrag();
  expect(dragGhost()).toHaveStyle({ transform: "translate3d(326px, 22px, 0)" });
  expect(screen.queryByRole("status", { name: "Aplicação do decorativo" })).not.toBeInTheDocument();
  h.view.rerender(<MediaPanel {...h.props} decorativeDropTarget={{ role: "overlay", scope: "left" }} />);
  const card = dragGhost()!;
  expect(screen.getByRole("status", { name: "Aplicação do decorativo" })).toHaveTextContent("SobreposiçãoLado esquerdo");
  expect(card).toContainElement(screen.getByRole("status", { name: "Aplicação do decorativo" }));
  // Padding and border keep the thumbnail where the bare ghost showed it.
  expect(card).toHaveStyle({ transform: "translate3d(321px, 17px, 0)" });
  h.view.rerender(<MediaPanel {...h.props} decorativeDropTarget={null} />);
  expect(screen.queryByRole("status", { name: "Aplicação do decorativo" })).not.toBeInTheDocument();
  dropDrag();
});

test("a right click during a drag opens no menu and the drag still drops", async () => {
  const h = harness(); dragHit(null); startDrag(); moveDrag();
  fireEvent.contextMenu(screen.getByRole("button", { name: "001.jpg" }), { button: 2, clientX: 320, clientY: 16 });
  expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  expect(dragGhost()).not.toBeNull();
  dropDrag();
  expect(h.props.onMediaDragChange).toHaveBeenLastCalledWith(expect.objectContaining({ phase: "drop" }));
  await act(() => new Promise((resolve) => window.setTimeout(resolve, 0)));
  fireEvent.contextMenu(screen.getByRole("button", { name: "001.jpg" }), { button: 2 });
  expect(screen.getByRole("menu", { name: "Ações das imagens" })).toBeInTheDocument();
});

test("a missing photo without observed dimensions keeps a visible placeholder during drag", () => {
  const h = harness();
  h.view.rerender(<MediaPanel {...h.props}
    mediaItems={[{ ...items[0], sourceWidthPx: 1, sourceHeightPx: 1 }, ...items.slice(1)]}
    mediaFiles={{ p1: { mediaId: "p1", state: "absent", createdAtMs: null, modifiedAtMs: null } }} />);
  dragHit(null); startDrag(/001.jpg/); moveDrag();
  expect(dragGhost()).toHaveStyle({ width: "60px", height: "60px" });
  expect(dragGhost()!.querySelector(".media-preview-thumbnail__missing-symbol")).not.toBeNull();
  dropDrag();
});

test.each([true, false])("a Decorative uses its preview dimensions when already loaded: %s", (alreadyLoaded) => {
  const h = harness();
  const url = "http://myalbuns-cache.localhost/portrait.png";
  h.view.rerender(<MediaPanel {...h.props}
    mediaItems={[...items.slice(0, 2), { ...items[2], sourceWidthPx: null, sourceHeightPx: null }]}
    previewSource={{ kind: "static", previews: { d1: { mediaId: "d1", state: "ready", url } } }} />);
  fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));
  const loadPortrait = (image: HTMLImageElement) => {
    Object.defineProperties(image, {
      complete: { value: true }, naturalWidth: { value: 400 }, naturalHeight: { value: 600 }, currentSrc: { value: url },
    });
    fireEvent.load(image);
  };
  if (alreadyLoaded) loadPortrait(screen.getByRole("button", { name: "Fundo.jpg" }).querySelector("img")!);
  dragHit(null); startDrag("Fundo.jpg"); moveDrag();
  if (!alreadyLoaded) loadPortrait(dragGhost()!.querySelector("img")!);
  expect(dragGhost()).toHaveStyle({ width: "40px", height: "60px" });
  expect(dragGhost()!.querySelector("img")).toHaveAttribute("src", url);
  dropDrag();
});

test.each([true, false])("missing photo ghost preserves its cached preview when available: %s", (cached) => {
  const h = harness();
  h.view.rerender(<MediaPanel {...h.props}
    mediaItems={[{ ...items[0], sourceWidthPx: 400, sourceHeightPx: 600 }, ...items.slice(1)]}
    mediaFiles={{ p1: { mediaId: "p1", state: "absent", createdAtMs: null, modifiedAtMs: null } }}
    previewSource={{ kind: "static", previews: {
      p1: { mediaId: "p1", state: "absent", url: cached ? "http://myalbuns-cache.localhost/retained.jpg" : null },
    } }} />);
  dragHit(null); startDrag(/001.jpg/); moveDrag();
  const ghost = dragGhost()!;
  expect(parseFloat(ghost.style.height)).toBe(60);
  expect(parseFloat(ghost.style.width)).toBe(40);
  if (cached) expect(ghost.querySelector("img")).toHaveAttribute("src", "http://myalbuns-cache.localhost/retained.jpg");
  else expect(ghost.querySelector(".media-preview-thumbnail")).toHaveAttribute("data-missing", "true");
  dropDrag(); expect(dragGhost()).toBeNull();
});

test.each(["hidden", "unmount"])("%s removes the ghost and cancels the shared gesture", (reason) => {
  const h = harness(); dragHit(null); startDrag(); moveDrag();
  expect(dragGhost()).not.toBeNull();
  if (reason === "hidden") h.view.rerender(<MediaPanel {...h.props} hidden />);
  else h.view.unmount();
  expect(dragGhost()).toBeNull();
  expect(h.props.onMediaDragChange).toHaveBeenLastCalledWith(null);
});

test("dragging to a folder moves only the dragged photo once without activating the folder or dropping on Canvas", () => {
  const h = harness();
  fireEvent.click(screen.getByRole("button", { name: "002.jpg" }));
  const target = screen.getByRole("button", { name: /Coleção Turma B/ });
  dragHit(target.querySelector("small"));
  startDrag();
  fireEvent.pointerMove(document, { pointerId: 7, clientX: 22, clientY: 101 });
  expect(target).not.toHaveClass("media-folder-chip--drop");
  moveDrag();
  expect(target).toHaveClass("media-folder-chip--drop");
  expect(dragGhost()).toHaveAttribute("data-media-drag-ghost", "p1");
  expect(h.edit).not.toHaveBeenCalled();
  dropDrag(); dropDrag();
  expect(dragGhost()).toBeNull();
  expect(h.edit).toHaveBeenCalledExactlyOnceWith({ kind: "moveMedia", mediaIds: ["p1"], folderId: "b" });
  expect(h.props.onMediaDragChange).toHaveBeenLastCalledWith(null);
  expect(h.props.onMediaDragChange.mock.calls.some(([drag]) => drag?.phase === "drop")).toBe(false);
  expect(target).not.toHaveClass("media-folder-chip--drop");
  fireEvent.click(target);
  expect(screen.getByRole("button", { name: "Todas 2" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("region", { name: "Painel de imagens" })).toHaveFocus();
});

test.each(["Escape", "pointercancel", "blur"])("%s cancels folder drag without an edit", (reason) => {
  const h = harness(); const target = screen.getByRole("button", { name: /Coleção Turma B/ });
  dragHit(target); startDrag(); moveDrag();
  expect(target).toHaveClass("media-folder-chip--drop");
  if (reason === "Escape") fireEvent.keyDown(window, { key: "Escape" });
  else if (reason === "blur") fireEvent.blur(window);
  else fireEvent.pointerCancel(document, { pointerId: 7 });
  dropDrag();
  expect(h.edit).not.toHaveBeenCalled();
  expect(target).not.toHaveClass("media-folder-chip--drop");
  expect(dragGhost()).toBeNull();
  expect(h.props.onMediaDragChange).toHaveBeenLastCalledWith(null);
});

test("leaving a folder forwards the existing Canvas drag and drop unchanged", () => {
  const h = harness(); const target = screen.getByRole("button", { name: /Coleção Turma B/ });
  const hit = dragHit(target); startDrag(); moveDrag();
  hit.mockReturnValue(null); moveDrag(); dropDrag();
  expect(target).not.toHaveClass("media-folder-chip--drop");
  expect(h.edit).not.toHaveBeenCalled();
  expect(h.props.onMediaDragChange).toHaveBeenLastCalledWith(expect.objectContaining({ mediaId: "p1", kind: "photo", phase: "drop", x: 320, y: 16 }));
});

test.each(["Todas 2", /^Ausentes/, "Nova coleção"])("%s is not a folder drop target", (name) => {
  const h = harness();
  h.view.rerender(<MediaPanel {...h.props} mediaFiles={{ p2: { mediaId: "p2", state: "absent", createdAtMs: null, modifiedAtMs: null } }} />);
  dragHit(screen.getByRole("button", { name })); startDrag(); moveDrag(); dropDrag();
  expect(h.edit).not.toHaveBeenCalled();
  expect(document.querySelector(".media-folder-chip--drop")).toBeNull();
});

test("blocking interactions during a folder drag cancels it and prevents another drag", () => {
  const h = harness(); dragHit(screen.getByRole("button", { name: /Coleção Turma B/ })); startDrag(); moveDrag();
  h.view.rerender(<MediaPanel {...h.props} relinkDisabled />);
  expect(dragGhost()).toBeNull();
  expect(document.querySelector(".media-folder-chip--drop")).toBeNull();
  dropDrag(); startDrag(); moveDrag(); dropDrag();
  expect(h.edit).not.toHaveBeenCalled();
});

test("a folder removed during drag cannot receive the photo", () => {
  const h = harness(); const target = screen.getByRole("button", { name: /Coleção Turma B/ });
  dragHit(target); startDrag(); moveDrag();
  h.view.rerender(<MediaPanel {...h.props} mediaFolders={[folders[0], folders[2]]} />);
  dropDrag(); expect(h.edit).not.toHaveBeenCalled();
});

test("moving out of a filtered folder keeps the filter and connected focus", () => {
  const h = harness();
  fireEvent.click(screen.getByRole("button", { name: /Coleção Turma A/ }));
  dragHit(screen.getByRole("button", { name: /Coleção Turma B/ })); startDrag(); moveDrag(); dropDrag();
  h.view.rerender(<MediaPanel {...h.props} mediaFolders={[
    { ...folders[0], mediaIds: [] }, { ...folders[1], mediaIds: ["p1", "p2"] }, folders[2],
  ]} />);
  expect(gridItems()).toEqual([]);
  expect(screen.getByRole("button", { name: /Coleção Turma A/ })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("region", { name: "Painel de imagens" })).toHaveFocus();
});

test("Decoratives and photos with missing originals can be organized by dragging", () => {
  const h = harness();
  h.view.rerender(<MediaPanel {...h.props} mediaFiles={{ p1: { mediaId: "p1", state: "absent", createdAtMs: null, modifiedAtMs: null } }} />);
  const hit = dragHit(screen.getByRole("button", { name: /Coleção Turma B/ }));
  startDrag(/001.jpg/); moveDrag(); dropDrag();
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "moveMedia", mediaIds: ["p1"], folderId: "b" });
  h.view.unmount();
  const decorative = harness(); fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));
  hit.mockReturnValue(screen.getByRole("button", { name: /Coleção Fundos/ }));
  startDrag("Fundo.jpg"); moveDrag(); dropDrag();
  expect(decorative.edit).toHaveBeenLastCalledWith({ kind: "moveMedia", mediaIds: ["d1"], folderId: "c" });
});

test("folders filter each tab independently and intersect search and absence", async () => {
  const h = harness();
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: /Coleção Turma A/ }));
  expect(gridItems()).toEqual(["p1"]);
  await user.type(screen.getByRole("searchbox"), "002");
  expect(gridItems()).toEqual([]);
  await user.clear(screen.getByRole("searchbox"));
  h.view.rerender(<MediaPanel {...h.props} mediaFiles={{ p1: { mediaId: "p1", state: "absent", createdAtMs: null, modifiedAtMs: null } }} />);
  await user.click(screen.getByRole("button", { name: /Ausentes/ }));
  expect(gridItems()).toEqual(["p1"]);
  await user.click(screen.getByRole("button", { name: "Decorativos" }));
  expect(screen.queryByRole("button", { name: /Coleção Turma A/ })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: /Coleção Fundos/ }));
  expect(gridItems()).toEqual(["d1"]);
  await user.click(screen.getByRole("button", { name: "Fotos" }));
  expect(gridItems()).toEqual(["p1"]);
  await user.click(screen.getByRole("button", { name: "Todas 2" }));
  expect(gridItems()).toEqual(["p1", "p2"]);
  expect(h.edit).not.toHaveBeenCalled();
});

test("+ names a new collection in place; invalid names stay open with the tooltip", async () => {
  const h = harness(); const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Nova coleção" }));
  const input = screen.getByRole("textbox", { name: "Nome da nova coleção" });
  expect(input).toHaveValue("Nova coleção");
  expect(input).toHaveFocus();
  await user.keyboard("turma a{Enter}");
  expect(await screen.findByRole("tooltip")).toHaveTextContent("Esse nome já está em uso nesta aba.");
  expect(input).toHaveAttribute("aria-invalid", "true");
  expect(h.edit).not.toHaveBeenCalled();
  await user.clear(input); await user.type(input, "Retratos{Enter}");
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "create", mediaKind: "photo", name: "Retratos" });
  await waitFor(() => expect(screen.queryByRole("textbox", { name: "Nome da nova coleção" })).not.toBeInTheDocument());
});

test("an unnamed new collection keeps the next free default name", async () => {
  const h = harness(); const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "Nova coleção" }));
  await user.keyboard("{Escape}");
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "create", mediaKind: "photo", name: "Nova coleção" });
  h.view.rerender(<MediaPanel {...h.props} mediaFolders={[...folders, { id: "n", kind: "photo", name: "Nova coleção", mediaIds: [] }]} />);
  await user.click(screen.getByRole("button", { name: "Nova coleção" }));
  const input = screen.getByRole("textbox", { name: "Nome da nova coleção" });
  expect(input).toHaveValue("Nova coleção (2)");
  await user.clear(input);
  fireEvent.blur(input);
  await waitFor(() => expect(h.edit).toHaveBeenLastCalledWith({ kind: "create", mediaKind: "photo", name: "Nova coleção (2)" }));
});

test("renaming happens on the chip from the menu, F2 or double click; Esc and invalid names restore it", async () => {
  const h = harness(); const user = userEvent.setup();
  fireEvent.contextMenu(screen.getByRole("button", { name: /Coleção Turma A/ }));
  await user.click(screen.getByRole("menuitem", { name: "Renomear" }));
  const rename = screen.getByRole("textbox", { name: "Nome da coleção Turma A" });
  expect(rename).toHaveValue("Turma A");
  await user.keyboard("{Escape}");
  expect(screen.getByRole("button", { name: /Coleção Turma A/ })).toHaveFocus();
  const chipB = screen.getByRole("button", { name: /Coleção Turma B/ });
  chipB.focus(); await user.keyboard("{F2}");
  await user.keyboard("Cerimônia{Enter}");
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "rename", folderId: "b", name: "Cerimônia" });
  await user.dblClick(screen.getByRole("button", { name: /Coleção Turma A/ }));
  const again = screen.getByRole("textbox", { name: "Nome da coleção Turma A" });
  await user.clear(again); await user.type(again, "turma a");
  fireEvent.blur(again);
  await waitFor(() => expect(screen.queryByRole("textbox", { name: "Nome da coleção Turma A" })).not.toBeInTheDocument());
  expect(h.edit).toHaveBeenCalledTimes(1);
});

test("the move submenu acts on the selection, marks the current collection and can create one", async () => {
  const h = harness(); const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "001.jpg" }));
  fireEvent.click(screen.getByRole("button", { name: "002.jpg" }), { ctrlKey: true });
  const openSubmenu = async () => {
    fireEvent.contextMenu(screen.getByRole("button", { name: "001.jpg" }));
    await user.click(screen.getByRole("menuitem", { name: "Mover para coleção" }));
    return screen.getByRole("menu", { name: "Mover para coleção" });
  };
  let submenu = await openSubmenu();
  expect(within(submenu).getByRole("menuitemcheckbox", { name: "Turma A" })).toHaveAttribute("aria-checked", "true");
  expect(within(submenu).getByRole("menuitemcheckbox", { name: "Sem coleção" })).toHaveAttribute("aria-checked", "false");
  await user.click(within(submenu).getByRole("menuitemcheckbox", { name: "Turma B" }));
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "moveMedia", mediaIds: ["p1", "p2"], folderId: "b" });
  submenu = await openSubmenu();
  await user.click(within(submenu).getByRole("menuitemcheckbox", { name: "Sem coleção" }));
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "moveMedia", mediaIds: ["p1", "p2"], folderId: null });
  submenu = await openSubmenu();
  await user.click(within(submenu).getByRole("menuitem", { name: "Nova coleção" }));
  expect(screen.getByRole("textbox", { name: "Nome da nova coleção" })).toHaveFocus();
  await user.keyboard("{Enter}");
  expect(h.edit).toHaveBeenLastCalledWith({ kind: "create", mediaKind: "photo", name: "Nova coleção", mediaIds: ["p1", "p2"] });
  expect(h.remove).not.toHaveBeenCalled();
});

test("the move submenu opens and closes from the keyboard", async () => {
  harness(); const user = userEvent.setup();
  fireEvent.contextMenu(screen.getByRole("button", { name: "001.jpg" }));
  const trigger = screen.getByRole("menuitem", { name: "Mover para coleção" });
  await act(async () => {}); // the menu focuses its first item first
  trigger.focus(); await user.keyboard("{ArrowRight}");
  const submenu = screen.getByRole("menu", { name: "Mover para coleção" });
  expect(within(submenu).getByRole("menuitemcheckbox", { name: "Sem coleção" })).toHaveFocus();
  await user.keyboard("{ArrowDown}");
  expect(within(submenu).getByRole("menuitemcheckbox", { name: "Turma A" })).toHaveFocus();
  await user.keyboard("{ArrowLeft}");
  expect(screen.queryByRole("menu", { name: "Mover para coleção" })).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();
  expect(trigger).toHaveAttribute("aria-expanded", "false");
});

test("deleting an active folder returns to All; undo does not revive the stale filter", async () => {
  const h = harness(); const user = userEvent.setup();
  const chip = screen.getByRole("button", { name: /Coleção Turma A/ });
  await user.click(chip); fireEvent.contextMenu(chip);
  await user.click(screen.getByRole("menuitem", { name: "Excluir coleção" }));
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
  await user.click(screen.getByRole("button", { name: /Coleção Turma A/ }));
  await user.click(screen.getByRole("button", { name: /^Todas/ }));
  expect(screen.getByRole("button", { name: "002.jpg" })).toHaveAttribute("aria-pressed", "false");
  const plan = h.ref.current!.planCatalog(items, [], [{ ...folders[0], mediaIds: ["p2"] }]);
  expect(plan.demand.visibleMediaIds).not.toContain("p1");
});

test("a pending name is sent once and the field waits for the result", async () => {
  const h = harness(); const user = userEvent.setup();
  let finish!: (result: boolean) => void;
  h.edit.mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  await user.click(screen.getByRole("button", { name: "Nova coleção" }));
  const input = screen.getByRole("textbox", { name: "Nome da nova coleção" });
  await user.keyboard("Retratos{Enter}");
  fireEvent.keyDown(input, { key: "Enter" }); fireEvent.blur(input);
  await waitFor(() => expect(input).toBeDisabled());
  expect(h.edit).toHaveBeenCalledTimes(1);
  await act(async () => finish(true));
  await waitFor(() => expect(screen.queryByRole("textbox", { name: "Nome da nova coleção" })).not.toBeInTheDocument());
});

test("losing an active folder clears its Ausentes filter and returns to All", async () => {
  const h = harness(); const user = userEvent.setup();
  h.view.rerender(<MediaPanel {...h.props} mediaFiles={{ p1: { mediaId: "p1", state: "absent", createdAtMs: null, modifiedAtMs: null } }} />);
  await user.click(screen.getByRole("button", { name: /Coleção Turma A/ }));
  await user.click(screen.getByRole("button", { name: /Ausentes/ }));
  expect(gridItems()).toEqual(["p1"]);
  h.view.rerender(<MediaPanel {...h.props} mediaFolders={folders.slice(1)}
    mediaFiles={{ p1: { mediaId: "p1", state: "absent", createdAtMs: null, modifiedAtMs: null } }} />);
  expect(screen.getByRole("button", { name: "Todas 2" })).toHaveAttribute("aria-pressed", "true");
  expect(gridItems()).toEqual(["p1", "p2"]);
});

test("Ausentes appears only while files are missing or its filter is on", async () => {
  const h = harness(); const user = userEvent.setup();
  const absent = { p1: { mediaId: "p1", state: "absent" as const, createdAtMs: null, modifiedAtMs: null } };
  expect(screen.queryByRole("button", { name: /Ausentes/ })).not.toBeInTheDocument();
  h.view.rerender(<MediaPanel {...h.props} mediaFiles={absent} />);
  await user.click(screen.getByRole("button", { name: /Ausentes/ }));
  h.view.rerender(<MediaPanel {...h.props} mediaFiles={{}} />);
  expect(screen.getByRole("button", { name: /Ausentes/ })).toHaveAttribute("aria-pressed", "true");
  await user.click(screen.getByRole("button", { name: /Ausentes/ }));
  expect(screen.queryByRole("button", { name: /Ausentes/ })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Todas 2" })).toHaveAttribute("aria-pressed", "true");
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
  await user.click(screen.getByRole("button", { name: /Coleção Turma A/ }));
  fireEvent.contextMenu(screen.getByRole("button", { name: "001.jpg" }));
  await user.click(screen.getByRole("menuitem", { name: "Mover para coleção" }));
  await user.click(screen.getByRole("menuitemcheckbox", { name: "Turma B" }));
  expect(gridItems()).toEqual([]);
  await waitFor(() => expect(screen.getByRole("region", { name: "Painel de imagens" })).toHaveFocus());
});

test("a refused save keeps the name open and focused", async () => {
  const h = harness(); const user = userEvent.setup();
  h.edit.mockResolvedValueOnce(false);
  await user.click(screen.getByRole("button", { name: "Nova coleção" }));
  const input = screen.getByRole("textbox", { name: "Nome da nova coleção" });
  await user.keyboard("Retratos{Enter}");
  await waitFor(() => expect(input).toBeEnabled());
  expect(input).toHaveFocus();
  await user.keyboard("{Enter}");
  await waitFor(() => expect(h.edit).toHaveBeenCalledTimes(2));
});

test("the Core normalized name is the one saved", async () => {
  const h = harness(); const user = userEvent.setup();
  h.validate.mockResolvedValueOnce({ name: "Nome normalizado pelo Core", error: null });
  await user.click(screen.getByRole("button", { name: "Nova coleção" }));
  await user.keyboard("  Nome enviado  {Enter}");
  expect(h.validate).toHaveBeenCalledExactlyOnceWith({ name: "Nome enviado", mediaKind: "photo", folderId: null });
  await waitFor(() => expect(h.edit).toHaveBeenCalledExactlyOnceWith({ kind: "create", mediaKind: "photo", name: "Nome normalizado pelo Core" }));
});

test("advice from an older document is asked again; a failed lookup stays in the tooltip", async () => {
  const h = harness(); const user = userEvent.setup();
  let resolve!: (result: MediaFolderNameValidation) => void;
  h.validate.mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
  await user.click(screen.getByRole("button", { name: "Nova coleção" }));
  const input = screen.getByRole("textbox", { name: "Nome da nova coleção" });
  await user.keyboard("Nova{Enter}");
  h.view.rerender(<MediaPanel {...h.props} folderValidationKey="next-revision" />);
  h.validate.mockResolvedValueOnce({ name: "Nova", error: "nameInUse" });
  await act(async () => resolve({ name: "Nova", error: null }));
  expect(h.validate).toHaveBeenCalledTimes(2);
  expect(await screen.findByRole("tooltip")).toHaveTextContent("Esse nome já está em uso nesta aba.");
  expect(h.edit).not.toHaveBeenCalled();
  h.validate.mockRejectedValueOnce(new Error("offline"));
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() => expect(screen.getByRole("tooltip")).toHaveTextContent("Não foi possível validar o nome"));
  expect(input).toHaveAttribute("aria-invalid", "true");
  expect(input).toHaveFocus();
  expect(h.edit).not.toHaveBeenCalled();
});
