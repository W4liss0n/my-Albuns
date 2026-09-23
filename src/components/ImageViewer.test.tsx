import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { ImageViewer } from "./ImageViewer";
import type { ViewerPresentation } from "../application/imageViewerWindow";
import { detectFaces } from "../image-viewer/faceLandmarks";

vi.mock("../image-viewer/faceLandmarks", async (original) => ({ ...(await original()), detectFaces: vi.fn() }));

const initial: ViewerPresentation = { sessionId: "s", revision: 1, mediaId: "a", name: "Imagem a", url: "data:image/png;id=a", state: "ready", canPrevious: false, canNext: true };
function Harness() {
  const [presentation, setPresentation] = useState(initial);
  return <ImageViewer presentation={presentation} onNavigate={() => setPresentation({ ...presentation, revision: presentation.revision + 1, mediaId: "b", name: "Imagem b", url: "data:image/png;id=b", canPrevious: true, canNext: false })} onClose={() => undefined} />;
}

test("navigation keeps geometry while a new preview loads and hides prior image", () => {
  render(<Harness />);
  const image = screen.getByRole("img", { name: "Imagem a" });
  Object.defineProperties(image, { naturalWidth: { configurable: true, value: 1600 }, naturalHeight: { configurable: true, value: 1200 } });
  fireEvent.load(image);
  fireEvent.click(screen.getByRole("button", { name: "Próxima imagem" }));
  expect(screen.getByRole("img", { name: "Imagem b" })).toHaveStyle({ opacity: "0" });
  expect(screen.getByText("Carregando imagem…")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Próxima imagem" })).toBeDisabled();
});

test("wheel zoom allows bounded pan and fit restores whole image", () => {
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(800);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(600);
  try {
    const view = render(<Harness />);
    const image = screen.getByRole("img", { name: "Imagem a" });
    Object.defineProperties(image, { naturalWidth: { configurable: true, value: 1600 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(image);
    const stage = view.container.querySelector<HTMLElement>(".image-viewer__stage")!;
    fireEvent.wheel(stage, { deltaY: -500, clientX: 400, clientY: 300 });
    expect(screen.getByRole("button", { name: "Ajustar à janela" })).toBeInTheDocument();
    stage.setPointerCapture = vi.fn(); stage.releasePointerCapture = vi.fn();
    fireEvent.pointerDown(stage, { pointerId: 1, button: 0, clientX: 200, clientY: 200 });
    fireEvent.pointerMove(stage, { pointerId: 1, clientX: 450, clientY: 350 });
    expect(image.style.transform).not.toContain("translate(0px, 0px)");
    fireEvent.pointerUp(stage, { pointerId: 1 });
    fireEvent.click(screen.getByRole("button", { name: "Ajustar à janela" }));
    expect(image.style.transform).toBe("translate(0px, 0px) scale(1)");
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("fit uses the stage padding for portrait geometry", () => {
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(442);
  const actualComputedStyle = window.getComputedStyle;
  const computedStyle = vi.spyOn(window, "getComputedStyle").mockImplementation((element) =>
    element.classList.contains("image-viewer__stage")
      ? { paddingLeft: "24px", paddingRight: "24px", paddingTop: "24px", paddingBottom: "24px" } as CSSStyleDeclaration
      : actualComputedStyle(element));
  try {
    render(<Harness />);
    const image = screen.getByRole("img", { name: "Imagem a" });
    Object.defineProperties(image, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(image);
    expect(parseFloat(image.style.width)).toBeCloseTo(394 * 800 / 1200);
    expect(parseFloat(image.style.height)).toBe(394);
    expect((442 - parseFloat(image.style.height)) / 2).toBe(24);
  } finally { width.mockRestore(); height.mockRestore(); computedStyle.mockRestore(); }
});


test("eye correction opens from a ready viewer and keeps the target fixed beside a project reference", () => {
  const onCorrection = vi.fn();
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "browse", referenceMediaId: "reference", referenceName: "Outra foto.jpg",
    referenceUrl: "data:image/png;id=reference", referenceState: "ready",
    canPreviousReference: false, canNextReference: true, resultUrl: null, error: null,
  } };
  const onNavigate = vi.fn();
  render(<ImageViewer presentation={presentation} onNavigate={onNavigate} onClose={vi.fn()} onCorrection={onCorrection} />);
  expect(screen.getByRole("img", { name: "Imagem a" })).toBeInTheDocument();
  expect(screen.getByRole("img", { name: "Outra foto.jpg" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Próxima referência" }));
  expect(onNavigate).toHaveBeenCalledWith(1);
  fireEvent.click(screen.getByRole("button", { name: "Usar esta foto" }));
  expect(onCorrection).toHaveBeenCalledWith({ sessionId: "s", kind: "select" });
});

test("a correction preview compares only the target and saves once even while showing the original", () => {
  const onCorrection = vi.fn();
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "preview", referenceMediaId: "reference", referenceName: "Outra foto.jpg",
    referenceUrl: "data:image/png;id=reference", referenceState: "ready",
    canPreviousReference: false, canNextReference: false,
    resultUrl: "data:image/png;id=corrected", error: null,
  } };
  render(<ImageViewer presentation={presentation} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  expect(screen.getByRole("img", { name: "Imagem a" })).toHaveAttribute("src", "data:image/png;id=corrected");
  expect(screen.getByText("Corrigida", { selector: ".eye-correction__pane-label" })).toBeInTheDocument();
  const reference = screen.getByRole("img", { name: "Outra foto.jpg" });
  const compare = screen.getByRole("button", { name: "Antes e depois: mostrar original" });
  expect(compare).toHaveAttribute("aria-pressed", "false");
  fireEvent.click(compare);
  expect(screen.getByRole("img", { name: "Imagem a" })).toHaveAttribute("src", initial.url);
  expect(screen.getByText("Original", { selector: ".eye-correction__pane-label" })).toBeInTheDocument();
  expect(reference).toHaveAttribute("src", "data:image/png;id=reference");
  expect(screen.getByRole("button", { name: "Antes e depois: mostrar correção" })).toHaveAttribute("aria-pressed", "true");
  expect(onCorrection).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Trocar referência" }));
  expect(onCorrection).toHaveBeenCalledWith({ sessionId: "s", kind: "browse" });
  const save = screen.getByRole("button", { name: "Salvar correção" });
  fireEvent.click(save);
  fireEvent.click(save);
  expect(onCorrection.mock.calls.filter(([action]) => action.kind === "apply")).toEqual([[{ sessionId: "s", kind: "apply" }]]);
  expect(save).toBeDisabled();
});

test("a changed correction result clears comparison without resetting target zoom", () => {
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "preview", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: "data:image/png;id=corrected-1", error: null,
  };
  const view = render(<ImageViewer presentation={{ ...initial, correction }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
  const target = screen.getByRole("img", { name: "Imagem a" });
  Object.defineProperties(target, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
  fireEvent.load(target);
  fireEvent.wheel(view.container.querySelectorAll(".eye-correction__pane-image")[1], { deltaY: -300 });
  const transform = target.style.transform;
  expect(transform).not.toContain("scale(1)");
  fireEvent.click(screen.getByRole("button", { name: "Antes e depois: mostrar original" }));
  expect(target).toHaveAttribute("src", initial.url);
  view.rerender(<ImageViewer presentation={{ ...initial, correction: { ...correction, resultUrl: "data:image/png;id=corrected-2" } }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
  expect(target).toHaveAttribute("src", "data:image/png;id=corrected-2");
  expect(target.style.transform).toBe(transform);
  expect(screen.getByRole("button", { name: "Antes e depois: mostrar original" })).toHaveAttribute("aria-pressed", "false");
});


test("an applying correction cannot be cancelled or navigated before commit settles", () => {
  const onCorrection = vi.fn();
  const onNavigate = vi.fn();
  const onClose = vi.fn();
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "applying", referenceMediaId: "reference", referenceName: "Outra foto.jpg",
    referenceUrl: "data:image/png;id=reference", referenceState: "ready",
    canPreviousReference: true, canNextReference: true,
    resultUrl: "data:image/png;id=corrected", error: null,
  } };
  render(<ImageViewer presentation={presentation} onNavigate={onNavigate} onClose={onClose} onCorrection={onCorrection} />);
  expect(screen.getByRole("button", { name: "Fechar correção" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Trocar referência" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Antes e depois: mostrar original" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Salvar correção" })).toBeDisabled();
  fireEvent.keyDown(window, { key: "Escape" });
  fireEvent.keyDown(window, { key: "ArrowRight" });
  expect(onCorrection).not.toHaveBeenCalled();
  expect(onNavigate).not.toHaveBeenCalled();
  expect(onClose).not.toHaveBeenCalled();
});

test("image tools show an accessible tooltip on keyboard focus and close on blur", async () => {
  render(<ImageViewer presentation={initial} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
  const image = screen.getByRole("img", { name: "Imagem a" });
  Object.defineProperties(image, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
  fireEvent.load(image);
  const tool = screen.getByRole("button", { name: "Abrir olhos" });
  expect(tool).not.toHaveAttribute("title");
  const user = userEvent.setup();
  await user.tab();
  await user.tab();
  expect(tool).toHaveFocus();
  expect(await screen.findByRole("tooltip")).toHaveTextContent("Abrir olhos");
  await user.tab();
  await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());
});

test("keyboard focus follows the eye tool into correction and returns when it closes", async () => {
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "browse", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  };
  function CorrectionHarness() {
    const [opened, setOpened] = useState(false);
    return <ImageViewer presentation={{ ...initial, correction: opened ? correction : undefined }} onNavigate={vi.fn()} onClose={vi.fn()}
      onCorrection={(action) => setOpened(action.kind === "start")} />;
  }
  render(<CorrectionHarness />);
  const image = screen.getByRole("img", { name: "Imagem a" });
  Object.defineProperties(image, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
  fireEvent.load(image);
  const open = screen.getByRole("button", { name: "Abrir olhos" });
  open.focus();
  fireEvent.click(open);
  const close = screen.getByRole("button", { name: "Fechar correção" });
  expect(close).toHaveFocus();
  fireEvent.click(close);
  expect(screen.getByRole("button", { name: "Abrir olhos" })).toHaveFocus();
});

test("correction tooltips follow focus and close when focus leaves the group", async () => {
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "preview", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: "data:image/png;id=corrected", error: null,
  };
  render(<ImageViewer presentation={{ ...initial, correction }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
  const compare = screen.getByRole("button", { name: "Antes e depois: mostrar original" });
  expect(compare).not.toHaveAttribute("title");
  compare.focus();
  expect(await screen.findByRole("tooltip")).toHaveTextContent("Antes e depois: mostrar original");
  screen.getByRole("button", { name: "Salvar correção" }).focus();
  await waitFor(() => expect(screen.getByRole("tooltip")).toHaveTextContent("Salvar correção"));
  screen.getByRole("img", { name: "Imagem a" }).parentElement?.focus();
  await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());
});

test("face markers expose selection and unlock preview only after both faces are chosen", async () => {
  const face = Array.from({ length: 264 }, () => ({ x: .5, y: .5, z: 0 }));
  vi.mocked(detectFaces).mockResolvedValue([face]);
  const onCorrection = vi.fn();
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "select", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  } };
  render(<ImageViewer presentation={presentation} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  for (const image of screen.getAllByRole("img")) {
    Object.defineProperties(image, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(image);
  }
  const target = await screen.findByRole("button", { name: "Imagem a corrigir: rosto 1" });
  const reference = screen.getByRole("button", { name: "Referência: rosto 1" });
  const preview = screen.getByRole("button", { name: "Ver correção" });
  expect(preview).toBeDisabled();
  fireEvent.click(target);
  expect(target).toHaveAttribute("aria-pressed", "true");
  expect(preview).toBeDisabled();
  fireEvent.click(reference);
  expect(reference).toHaveAttribute("aria-pressed", "true");
  expect(preview).toBeEnabled();
  fireEvent.click(preview);
  expect(onCorrection).toHaveBeenCalledWith(expect.objectContaining({ kind: "preview", targetFace: face, referenceFace: face }));
});

test("long correction errors expose the full message through the shared tooltip", async () => {
  const error = "Não foi possível preparar a correção para esta fotografia. Escolha outra referência e tente novamente.";
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "preview", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error,
  } };
  render(<ImageViewer presentation={presentation} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
  const hint = screen.getByRole("status");
  expect(hint).toHaveTextContent(error);
  expect(hint).not.toHaveAttribute("title");
  hint.focus();
  expect(hint).toHaveFocus();
  expect(await screen.findByRole("tooltip")).toHaveTextContent(error);
});
