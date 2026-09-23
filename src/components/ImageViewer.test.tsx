import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { ImageViewer } from "./ImageViewer";
import type { ViewerPresentation } from "../application/imageViewerWindow";
import { detectFaces } from "../image-viewer/faceLandmarks";

vi.mock("../image-viewer/faceLandmarks", async (original) => ({ ...(await original()), detectFaces: vi.fn() }));
beforeEach(() => { vi.mocked(detectFaces).mockReset().mockResolvedValue([]); });

const initial: ViewerPresentation = { sessionId: "s", revision: 1, mediaId: "a", name: "Imagem a", url: "data:image/png;id=a", state: "ready", canPrevious: false, canNext: true };
function Harness() {
  const [presentation, setPresentation] = useState(initial);
  return <ImageViewer presentation={presentation} onNavigate={() => setPresentation({ ...presentation, revision: presentation.revision + 1, mediaId: "b", name: "Imagem b", url: "data:image/png;id=b", canPrevious: true, canNext: false })} onClose={() => undefined} onCorrection={() => undefined} />;
}

test("navigation keeps the painted photo until the next image has geometry without exposing old actions", () => {
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(800);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(600);
  try {
    const view = render(<Harness />);
    const image = screen.getByRole("img", { name: "Imagem a" });
    Object.defineProperties(image, { naturalWidth: { configurable: true, value: 1600 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(image);
    fireEvent.click(screen.getByRole("button", { name: "Próxima imagem" }));
    expect(image.isConnected).toBe(true);
    expect(parseFloat(image.style.width)).toBeGreaterThan(0);
    expect(screen.queryByText("Carregando imagem…")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Abrir olhos" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Próxima imagem" })).toBeDisabled();
    const next = view.container.querySelector<HTMLImageElement>('img[src="data:image/png;id=b"]')!;
    Object.defineProperties(next, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
    fireEvent.load(next);
    expect(image.isConnected).toBe(false);
    expect(parseFloat(next.style.width)).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Abrir olhos" })).toBeInTheDocument();
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("reference navigation keeps its painted photo while another reference loads", () => {
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(500);
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "browse", referenceMediaId: "ref-a", referenceName: "Referência a.jpg", referenceUrl: "data:image/png;id=ref-a",
    referenceState: "ready", canPreviousReference: false, canNextReference: true, resultUrl: null, error: null,
  };
  try {
    const view = render(<ImageViewer presentation={{ ...initial, correction }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    const original = view.container.querySelector<HTMLImageElement>(".eye-correction__pane:first-child .eye-correction__photo img")!;
    Object.defineProperties(original, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(original);
    expect(parseFloat(original.parentElement!.style.width)).toBeGreaterThan(0);

    view.rerender(<ImageViewer presentation={{ ...initial, correction: { ...correction, referenceMediaId: "ref-b", referenceName: "Referência b.jpg", referenceUrl: "data:image/png;id=ref-b", canPreviousReference: true } }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    expect(original.isConnected).toBe(true);
    expect(parseFloat(original.parentElement!.style.width)).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "Usar esta foto" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("button", { name: "Próxima referência" })).toBeEnabled();

    const next = view.container.querySelector<HTMLImageElement>('img[src="data:image/png;id=ref-b"]')!;
    Object.defineProperties(next, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
    fireEvent.load(next);
    expect(original.isConnected).toBe(false);
    expect(parseFloat(next.parentElement!.style.width)).toBeGreaterThan(0);
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("loading without a URL and A to B to C navigation retain only the last painted photo and its zoom", () => {
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(800);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(600);
  const title = vi.fn();
  const props = { onNavigate: vi.fn(), onClose: vi.fn(), onCorrection: vi.fn(), onVisibleNameChange: title };
  try {
    const view = render(<ImageViewer presentation={initial} {...props} />);
    const first = view.container.querySelector<HTMLImageElement>(".image-viewer__image")!;
    Object.defineProperties(first, { naturalWidth: { configurable: true, value: 1600 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(first);
    fireEvent.wheel(view.container.querySelector(".image-viewer__stage")!, { deltaY: -500, clientX: 400, clientY: 300 });
    expect(first.style.transform).not.toContain("scale(1)");

    const loading: ViewerPresentation = { ...initial, mediaId: "b", name: "Imagem b", url: null, state: "loading" };
    view.rerender(<ImageViewer presentation={loading} {...props} />);
    expect(first.isConnected).toBe(true);
    expect(first.style.transform).not.toContain("scale(1)");
    expect(screen.queryByText("Carregando imagem…")).not.toBeInTheDocument();
    const zoomBeforeKey = first.style.transform;
    fireEvent.keyDown(window, { key: "+" });
    expect(first.style.transform).toBe(zoomBeforeKey);
    expect(title).toHaveBeenLastCalledWith("s", "Imagem a");

    const second: ViewerPresentation = { ...loading, url: "data:image/png;id=b", state: "ready" };
    view.rerender(<ImageViewer presentation={second} {...props} />);
    const pending = view.container.querySelector<HTMLImageElement>('img[src="data:image/png;id=b"]')!;
    const third: ViewerPresentation = { ...second, mediaId: "c", name: "Imagem c", url: "data:image/png;id=c" };
    view.rerender(<ImageViewer presentation={third} {...props} />);
    expect(first.isConnected).toBe(true);
    fireEvent.load(pending);
    expect(first.isConnected).toBe(true);
    const finalImage = view.container.querySelector<HTMLImageElement>('img[src="data:image/png;id=c"]')!;
    Object.defineProperties(finalImage, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
    fireEvent.load(finalImage);
    expect(first.isConnected).toBe(false);
    expect(finalImage.style.transform).toContain("scale(1)");
    expect(title).toHaveBeenLastCalledWith("s", "Imagem c");
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("missing or failed next photo releases the retained image without reviving it later", () => {
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(800);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(600);
  const props = { onNavigate: vi.fn(), onClose: vi.fn(), onCorrection: vi.fn() };
  try {
    const view = render(<ImageViewer presentation={initial} {...props} />);
    const first = view.container.querySelector<HTMLImageElement>(".image-viewer__image")!;
    Object.defineProperties(first, { naturalWidth: { configurable: true, value: 1600 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(first);
    view.rerender(<ImageViewer presentation={{ ...initial, mediaId: "b", name: "Imagem b", url: "data:image/png;id=b" }} {...props} />);
    expect(first.isConnected).toBe(true);
    fireEvent.error(view.container.querySelector('img[src="data:image/png;id=b"]')!);
    expect(first.isConnected).toBe(false);
    expect(screen.getByText("Não foi possível exibir a prévia desta imagem.")).toBeInTheDocument();
    view.rerender(<ImageViewer presentation={{ ...initial, mediaId: "c", name: "Imagem c", url: null, state: "absent" }} {...props} />);
    expect(view.container.querySelector(".image-viewer__image--retained")).not.toBeInTheDocument();
    expect(screen.getByText("Imagem não encontrada. Localize o arquivo pelo Painel de imagens.")).toBeInTheDocument();
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("reference loading, failure and missing states cannot revive an earlier photo", () => {
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(500);
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "browse", referenceMediaId: "ref-a", referenceName: "Referência a.jpg", referenceUrl: "data:image/png;id=ref-a",
    referenceState: "ready", canPreviousReference: false, canNextReference: true, resultUrl: null, error: null,
  };
  const props = { onNavigate: vi.fn(), onClose: vi.fn(), onCorrection: vi.fn() };
  try {
    const view = render(<ImageViewer presentation={{ ...initial, correction }} {...props} />);
    const first = view.container.querySelector<HTMLImageElement>('img[src="data:image/png;id=ref-a"]')!;
    Object.defineProperties(first, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(first);
    const loading: typeof correction = { ...correction, referenceMediaId: "ref-b", referenceName: "Referência b.jpg", referenceUrl: null, referenceState: "loading", canPreviousReference: true };
    view.rerender(<ImageViewer presentation={{ ...initial, correction: loading }} {...props} />);
    expect(first.isConnected).toBe(true);
    expect(screen.getByRole("button", { name: "Usar esta foto" })).toHaveAttribute("aria-disabled", "true");
    const next: typeof correction = { ...loading, referenceUrl: "data:image/png;id=ref-b", referenceState: "ready" };
    view.rerender(<ImageViewer presentation={{ ...initial, correction: next }} {...props} />);
    expect(first.isConnected).toBe(true);
    fireEvent.error(view.container.querySelector('img[src="data:image/png;id=ref-b"]')!);
    expect(first.isConnected).toBe(false);
    const missing: typeof correction = { ...next, referenceMediaId: "ref-c", referenceUrl: null, referenceState: "absent" };
    view.rerender(<ImageViewer presentation={{ ...initial, correction: missing }} {...props} />);
    expect(view.container.querySelector('[data-retained="true"]')).not.toBeInTheDocument();
    const later: typeof correction = { ...missing, referenceMediaId: "ref-d", referenceUrl: null, referenceState: "loading" };
    view.rerender(<ImageViewer presentation={{ ...initial, correction: later }} {...props} />);
    expect(view.container.querySelector('[data-retained="true"]')).not.toBeInTheDocument();
  } finally { width.mockRestore(); height.mockRestore(); }
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
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(500);
  try {
    const onCorrection = vi.fn();
    const presentation: ViewerPresentation = { ...initial, correction: {
      phase: "browse", referenceMediaId: "reference", referenceName: "Outra foto.jpg",
      referenceUrl: "data:image/png;id=reference", referenceState: "ready",
      canPreviousReference: false, canNextReference: true, resultUrl: null, error: null,
    } };
    const onNavigate = vi.fn();
    const view = render(<ImageViewer presentation={presentation} onNavigate={onNavigate} onClose={vi.fn()} onCorrection={onCorrection} />);
    expect(screen.getByRole("img", { name: "Imagem a" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Outra foto.jpg" })).toBeInTheDocument();
    expect(screen.queryByText("Imagem a")).not.toBeInTheDocument();
    expect(screen.queryByText("Outra foto.jpg")).not.toBeInTheDocument();
    expect(view.container.querySelector(".eye-correction__pane:first-child .eye-correction__reference-action")).toBe(screen.getByRole("button", { name: "Usar esta foto" }));
    expect(view.container.querySelector(".eye-correction__tools .eye-correction__reference-action")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Próxima referência" }));
    expect(onNavigate).toHaveBeenCalledWith(1);
    expect(screen.getByRole("button", { name: "Usar esta foto" })).toHaveAttribute("aria-disabled", "true");
    const reference = screen.getByRole("img", { name: "Outra foto.jpg" });
    Object.defineProperties(reference, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(reference);
    fireEvent.click(screen.getByRole("button", { name: "Usar esta foto" }));
    expect(onCorrection).toHaveBeenCalledWith({ sessionId: "s", kind: "select" });
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("a correction preview compares only the target and saves once even while showing the original", () => {
  const onCorrection = vi.fn();
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "preview", referenceMediaId: "reference", referenceName: "Outra foto.jpg",
    referenceUrl: "data:image/png;id=reference", referenceState: "ready",
    canPreviousReference: false, canNextReference: false,
    resultUrl: "data:image/png;id=corrected", error: null,
  } };
  const view = render(<ImageViewer presentation={presentation} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  expect(screen.getByRole("img", { name: "Imagem a" })).toHaveAttribute("src", "data:image/png;id=corrected");
  const reference = screen.getByRole("img", { name: "Outra foto.jpg" });
  const compare = screen.getByRole("button", { name: "Antes e depois: mostrar original" });
  expect(compare).toHaveAttribute("aria-pressed", "false");
  fireEvent.click(compare);
  expect(screen.getByRole("img", { name: "Imagem a" })).toHaveAttribute("src", initial.url);
  expect(reference).toHaveAttribute("src", "data:image/png;id=reference");
  expect(view.container.querySelector(".eye-correction__pane:first-child .eye-correction__reference-action")).toBe(screen.getByRole("button", { name: "Trocar referência" }));
  expect(screen.getByRole("button", { name: "Antes e depois: mostrar correção" })).toHaveAttribute("aria-pressed", "true");
  expect(onCorrection).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Trocar referência" }));
  expect(onCorrection).toHaveBeenCalledWith({ sessionId: "s", kind: "browse" });
  const save = screen.getByRole("button", { name: "Salvar correção" });
  fireEvent.click(save);
  expect(screen.getByRole("dialog", { name: "Substituir foto original?" })).toBeInTheDocument();
  expect(screen.getByText("A foto Imagem a será substituída pela versão corrigida.")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Cancelar" })).toHaveFocus();
  fireEvent.click(screen.getByRole("button", { name: "Substituir original" }));
  fireEvent.click(save);
  expect(onCorrection.mock.calls.filter(([action]) => action.kind === "apply")).toEqual([[{ sessionId: "s", kind: "apply" }]]);
});

test("a changed correction result clears comparison without resetting target zoom", () => {
  vi.mocked(detectFaces).mockResolvedValue([]);
  const paneWidth = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(800);
  const paneHeight = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(600);
  try {
    const correction: NonNullable<ViewerPresentation["correction"]> = {
      phase: "preview", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
      referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: "data:image/png;id=corrected-1", error: null,
    };
    const view = render(<ImageViewer presentation={{ ...initial, correction }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    const target = view.container.querySelector<HTMLImageElement>(".eye-correction__pane:last-child .eye-correction__photo img:first-child")!;
    Object.defineProperties(target, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
    fireEvent.load(target);
    fireEvent.wheel(view.container.querySelectorAll(".eye-correction__pane-image")[1], { deltaY: -300 });
    const photo = view.container.querySelectorAll<HTMLElement>(".eye-correction__photo")[1];
    const width = photo.style.width;
    expect(parseFloat(width)).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: "Antes e depois: mostrar original" }));
    expect(target).toHaveAttribute("src", initial.url);
    view.rerender(<ImageViewer presentation={{ ...initial, correction: { ...correction, resultUrl: "data:image/png;id=corrected-2" } }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    expect(screen.getByRole("img", { name: "Imagem a" })).toHaveAttribute("src", "data:image/png;id=corrected-2");
    expect(photo.style.width).toBe(width);
    expect(screen.getByRole("button", { name: "Antes e depois: mostrar original" })).toHaveAttribute("aria-pressed", "false");
  } finally { paneWidth.mockRestore(); paneHeight.mockRestore(); }
});

test("cancelled overwrite keeps the prepared preview and a stale confirmation cannot apply", () => {
  const onCorrection = vi.fn();
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "preview", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: "data:image/png;id=corrected-1", error: null,
  };
  const view = render(<ImageViewer presentation={{ ...initial, correction }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  fireEvent.click(screen.getByRole("button", { name: "Salvar correção" }));
  expect(screen.getByRole("button", { name: "Cancelar" })).toHaveFocus();
  fireEvent.keyDown(window, { key: "Escape" });
  expect(screen.queryByRole("dialog", { name: "Substituir foto original?" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Salvar correção" })).toHaveFocus();
  expect(screen.getByRole("img", { name: "Imagem a" })).toHaveAttribute("src", correction.resultUrl);
  expect(onCorrection).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Salvar correção" }));
  view.rerender(<ImageViewer presentation={{ ...initial, correction: { ...correction, resultUrl: "data:image/png;id=corrected-2" } }}
    onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  expect(screen.queryByRole("dialog", { name: "Substituir foto original?" })).not.toBeInTheDocument();
  expect(onCorrection).not.toHaveBeenCalled();
});

test("returning from correction measures the replacement stage and fits navigated images", () => {
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(800);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(600);
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "browse", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  };
  function CorrectionHarness() {
    const [current, setCurrent] = useState(initial);
    return <ImageViewer presentation={current} onNavigate={() => setCurrent((value) => ({ ...value, mediaId: "b", name: "Imagem b", url: "data:image/png;id=b" }))}
      onClose={vi.fn()} onCorrection={(action) => setCurrent((value) => ({ ...value, correction: action.kind === "start" ? correction : undefined }))} />;
  }
  try {
    render(<CorrectionHarness />);
    const image = screen.getByRole("img", { name: "Imagem a" });
    Object.defineProperties(image, { naturalWidth: { configurable: true, value: 1600 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(image);
    fireEvent.click(screen.getByRole("button", { name: "Abrir olhos" }));
    fireEvent.click(screen.getByRole("button", { name: "Fechar correção" }));
    const returned = screen.getByRole("img", { name: "Imagem a" });
    expect(parseFloat(returned.style.width)).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: "Próxima imagem" }));
    const next = document.querySelector<HTMLImageElement>('img[src="data:image/png;id=b"]')!;
    Object.defineProperties(next, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
    fireEvent.load(next);
    expect(parseFloat(next.style.width)).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole("button", { name: "Abrir olhos" }));
    fireEvent.click(screen.getByRole("button", { name: "Fechar correção" }));
    expect(parseFloat(screen.getByRole("img", { name: "Imagem b" }).style.width)).toBeGreaterThan(0);
  } finally { width.mockRestore(); height.mockRestore(); }
});


test("the displayed photo stays painted until correction geometry is ready, then closing restores it immediately", async () => {
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "browse", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=slow-reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  };
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(500);
  try {
    const view = render(<ImageViewer presentation={initial} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    const displayed = view.container.querySelector<HTMLImageElement>(".image-viewer__image")!;
    Object.defineProperties(displayed, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
    fireEvent.load(displayed);
    expect(parseFloat(displayed.style.width)).toBeGreaterThan(0);

    view.rerender(<ImageViewer presentation={{ ...initial, correction }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    expect(displayed.isConnected).toBe(true);
    expect(getComputedStyle(displayed.closest(".image-viewer__stage")!).visibility).toBe("visible");
    expect(parseFloat(displayed.style.width)).toBeGreaterThan(0);

    const target = view.container.querySelector<HTMLImageElement>(".eye-correction__pane:last-child .eye-correction__photo img")!;
    expect(parseFloat(target.parentElement!.style.width)).toBe(0);
    Object.defineProperties(target, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
    fireEvent.load(target);
    await waitFor(() => expect(parseFloat(target.parentElement!.style.width)).toBeGreaterThan(0));
    await waitFor(() => expect(getComputedStyle(displayed.closest(".image-viewer__stage")!).visibility).toBe("hidden"));
    // A slow reference does not hold the viewer on the old photo.
    expect(view.container.querySelector(".eye-correction__pane:first-child .eye-correction__photo img")).toBeInTheDocument();

    view.rerender(<ImageViewer presentation={initial} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    expect(displayed.isConnected).toBe(true);
    expect(getComputedStyle(displayed.closest(".image-viewer__stage")!).visibility).toBe("visible");
    expect(parseFloat(displayed.style.width)).toBeGreaterThan(0);
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("a quick correction exit and a failed target load never strand or misidentify the normal photo", () => {
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "browse", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=slow-reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  };
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(500);
  try {
    const view = render(<ImageViewer presentation={initial} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    const displayed = view.container.querySelector<HTMLImageElement>(".image-viewer__image")!;
    Object.defineProperties(displayed, { naturalWidth: { configurable: true, value: 1200 }, naturalHeight: { configurable: true, value: 800 } });
    fireEvent.load(displayed);
    const correcting = { ...initial, correction };
    view.rerender(<ImageViewer presentation={correcting} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    expect(getComputedStyle(displayed.closest(".image-viewer__stage")!).visibility).toBe("visible");
    view.rerender(<ImageViewer presentation={initial} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    expect(displayed.isConnected).toBe(true);
    expect(getComputedStyle(displayed.closest(".image-viewer__stage")!).visibility).toBe("visible");

    view.rerender(<ImageViewer presentation={correcting} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    const target = view.container.querySelector<HTMLImageElement>(".eye-correction__pane:last-child .eye-correction__photo img")!;
    fireEvent.error(target);
    expect(getComputedStyle(displayed.closest(".image-viewer__stage")!).visibility).toBe("hidden");

    const anotherSession = { ...initial, sessionId: "new-session", correction };
    view.rerender(<ImageViewer presentation={anotherSession} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    expect(displayed.isConnected).toBe(false);
    expect(getComputedStyle(view.container.querySelector(".image-viewer__stage")!).visibility).toBe("hidden");

    const different = { ...initial, sessionId: "new-session", mediaId: "b", name: "Imagem b", url: "data:image/png;id=b", correction };
    view.rerender(<ImageViewer presentation={different} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    expect(view.container.querySelector<HTMLImageElement>(".image-viewer__image")).toHaveAttribute("src", different.url);
    expect(getComputedStyle(view.container.querySelector(".image-viewer__stage")!).visibility).toBe("hidden");
  } finally { width.mockRestore(); height.mockRestore(); }
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
  const view = render(<ImageViewer presentation={presentation} onNavigate={onNavigate} onClose={onClose} onCorrection={onCorrection} />);
  expect(screen.getByRole("button", { name: "Fechar correção" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Trocar referência" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Antes e depois: mostrar original" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Salvar correção" })).toBeDisabled();
  expect(view.container.querySelector(".eye-correction__pane:first-child .eye-correction__reference-action")).toBe(screen.getByRole("button", { name: "Trocar referência" }));
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
  screen.getByRole("img", { name: "Imagem a" }).closest<HTMLElement>(".eye-correction__pane-image")?.focus();
  await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());
});

test("face bounds track fit and zoom, clip off-image faces, and allow pan without accidental selection", async () => {
  const edge = [{ x: .03, y: .25, z: 0 }, { x: .15, y: .55, z: 0 }];
  const center = [{ x: .4, y: .25, z: 0 }, { x: .6, y: .55, z: 0 }];
  vi.mocked(detectFaces).mockResolvedValue([edge, center]);
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(500);
  const onCorrection = vi.fn();
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "select", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  } };
  try {
    const view = render(<ImageViewer presentation={presentation} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
    for (const image of screen.getAllByRole("img")) {
      Object.defineProperties(image, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
      fireEvent.load(image);
    }
    const target = await screen.findByRole("button", { name: "Imagem a corrigir: rosto 2" });
    const reference = screen.getByRole("button", { name: "Referência: rosto 1" });
    const pane = view.container.querySelectorAll<HTMLElement>(".eye-correction__pane-image")[1];
    const photo = pane.querySelector<HTMLElement>(".eye-correction__photo")!;
    const fittedWidth = (500 - 48) * 800 / 1200;
    expect(parseFloat(photo.style.width)).toBeCloseTo(fittedWidth);
    expect(parseFloat(photo.style.left) + .4 * parseFloat(photo.style.width)).toBeCloseTo(300 - .1 * fittedWidth);
    expect(parseFloat(target.style.left)).toBeCloseTo(40);
    expect(parseFloat(target.style.width)).toBeCloseTo(20);
    expect(screen.queryByRole("button", { name: "Ver correção" })).not.toBeInTheDocument();
    fireEvent.click(reference);
    expect(reference).toHaveAttribute("aria-pressed", "true");
    expect(reference.querySelector(".eye-correction__face-check")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Referência: rosto 2" }).querySelector(".eye-correction__face-check")).not.toBeInTheDocument();
    expect(target.querySelector(".eye-correction__face-check")).not.toBeInTheDocument();
    fireEvent.wheel(pane, { deltaY: -700 });
    expect(screen.queryByRole("button", { name: "Imagem a corrigir: rosto 1" })).not.toBeInTheDocument();
    expect(parseFloat(photo.style.width)).toBeGreaterThan(fittedWidth * 4);
    const beforePan = parseFloat(photo.style.left);
    pane.setPointerCapture = vi.fn(); pane.releasePointerCapture = vi.fn();
    fireEvent.pointerDown(target, { pointerId: 1, button: 0, clientX: 200, clientY: 250 });
    expect(pane.setPointerCapture).not.toHaveBeenCalled();
    fireEvent.pointerMove(pane, { pointerId: 1, clientX: 250, clientY: 250 });
    expect(pane.setPointerCapture).toHaveBeenCalledWith(1);
    fireEvent.pointerUp(pane, { pointerId: 1 });
    expect(parseFloat(photo.style.left)).toBeCloseTo(beforePan + 50);
    fireEvent.click(target);
    expect(target).toHaveAttribute("aria-pressed", "false");
    fireEvent.pointerDown(target, { pointerId: 2, button: 0, clientX: 200, clientY: 250 });
    expect(pane.setPointerCapture).toHaveBeenCalledTimes(1);
    fireEvent.pointerUp(target, { pointerId: 2 });
    fireEvent.click(target);
    expect(target).toHaveAttribute("aria-pressed", "true");
    expect(target.querySelector(".eye-correction__face-check")).toBeInTheDocument();
    await waitFor(() => expect(onCorrection).toHaveBeenCalledWith(expect.objectContaining({ kind: "preview", targetFace: center, referenceFace: edge })));
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("a missing destination face blocks reference selection and explains why on the photo", async () => {
  vi.mocked(detectFaces).mockResolvedValue([]);
  const onCorrection = vi.fn();
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "browse", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  } };
  const view = render(<ImageViewer presentation={presentation} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  const target = screen.getByRole("img", { name: "Imagem a" });
  Object.defineProperties(target, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
  fireEvent.load(target);
  await waitFor(() => expect(view.container.querySelector(".eye-correction__pane:last-child [data-analysis='no-face']")).toBeInTheDocument());
  const choose = screen.getByRole("button", { name: "Usar esta foto" });
  await waitFor(() => expect(choose).toHaveAttribute("aria-disabled", "true"));
  screen.getByRole("group", { name: "Imagem a corrigir: aviso" }).focus();
  expect(await screen.findByRole("tooltip")).toHaveTextContent("Nenhum rosto encontrado na foto de destino.");
  screen.getByRole("group", { name: "Imagem a corrigir: aviso" }).blur();
  await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());
  fireEvent.keyDown(choose, { key: "Enter" });
  fireEvent.keyDown(choose, { key: " " });
  fireEvent.click(choose);
  expect(onCorrection).not.toHaveBeenCalled();
  expect(view.container.querySelector(".eye-correction__hint")).not.toBeInTheDocument();
});

test("zoom centers the selected face without zooming on selection and keeps the level when another face is chosen", async () => {
  const first = [{ x: .57, y: .24, z: 0 }, { x: .63, y: .36, z: 0 }];
  const second = [{ x: .65, y: .24, z: 0 }, { x: .71, y: .36, z: 0 }];
  vi.mocked(detectFaces).mockResolvedValue([first, second]);
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(500);
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "select", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  };
  try {
    const view = render(<ImageViewer presentation={{ ...initial, correction }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
    for (const image of screen.getAllByRole("img")) {
      Object.defineProperties(image, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
      fireEvent.load(image);
    }
    const pane = view.container.querySelectorAll<HTMLElement>(".eye-correction__pane-image")[1];
    const photo = pane.querySelector<HTMLElement>(".eye-correction__photo")!;
    fireEvent.click(await screen.findByRole("button", { name: "Imagem a corrigir: rosto 1" }));
    const fittedWidth = parseFloat(photo.style.width);
    expect(screen.queryByRole("button", { name: "Ajustar imagem a corrigir à janela" })).not.toBeInTheDocument();
    fireEvent.wheel(pane, { deltaY: -700 });
    const zoomedWidth = parseFloat(photo.style.width);
    expect(zoomedWidth).toBeGreaterThan(fittedWidth * 3);
    expect(parseFloat(photo.style.left) + .6 * zoomedWidth).toBeCloseTo(300);
    fireEvent.click(screen.getByRole("button", { name: "Imagem a corrigir: rosto 2" }));
    expect(parseFloat(photo.style.width)).toBeCloseTo(zoomedWidth);
    expect(parseFloat(photo.style.left) + .68 * zoomedWidth).toBeCloseTo(300);
    fireEvent.keyDown(pane, { key: "+" });
    expect(parseFloat(photo.style.left) + .68 * parseFloat(photo.style.width)).toBeCloseTo(300);
    for (let step = 0; step < 12; step++) fireEvent.keyDown(pane, { key: "-" });
    expect(parseFloat(photo.style.width)).toBeCloseTo(fittedWidth);
    expect(parseFloat(photo.style.left)).toBeCloseTo((600 - fittedWidth) / 2);
    fireEvent.wheel(pane, { deltaY: -700 });
    fireEvent.keyDown(pane, { key: "0" });
    expect(parseFloat(photo.style.width)).toBeCloseTo(fittedWidth);
    expect(parseFloat(photo.style.left)).toBeCloseTo((600 - fittedWidth) / 2);
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("face boxes and original-image analysis survive processing, preview and comparison while a new pair replaces the old result", async () => {
  vi.mocked(detectFaces).mockClear();
  const first = [{ x: .35, y: .24, z: 0 }, { x: .45, y: .36, z: 0 }];
  const second = [{ x: .55, y: .24, z: 0 }, { x: .65, y: .36, z: 0 }];
  vi.mocked(detectFaces).mockResolvedValue([first, second]);
  const correction: NonNullable<ViewerPresentation["correction"]> = {
    phase: "select", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  };
  const onCorrection = vi.fn();
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(500);
  try {
    const view = render(<ImageViewer presentation={{ ...initial, correction }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
    for (const image of screen.getAllByRole("img")) {
      Object.defineProperties(image, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
      fireEvent.load(image);
    }
    fireEvent.click(await screen.findByRole("button", { name: "Imagem a corrigir: rosto 1" }));
    fireEvent.click(screen.getByRole("button", { name: "Referência: rosto 1" }));
    await waitFor(() => expect(onCorrection.mock.calls.filter(([action]) => action.kind === "preview")).toHaveLength(1));
    expect(vi.mocked(detectFaces)).toHaveBeenCalledTimes(2);
    view.rerender(<ImageViewer presentation={{ ...initial, correction: { ...correction, phase: "processing" } }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
    expect(screen.getByRole("button", { name: "Imagem a corrigir: rosto 1" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "Referência: rosto 1" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: "Imagem a corrigir: rosto 2" }));
    await waitFor(() => expect(onCorrection.mock.calls.filter(([action]) => action.kind === "preview")).toHaveLength(2));
    expect(screen.queryByRole("button", { name: "Salvar correção" })).not.toBeInTheDocument();
    view.rerender(<ImageViewer presentation={{ ...initial, correction: { ...correction, phase: "preview", resultUrl: "data:image/png;id=corrected-1" } }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
    expect(screen.getByRole("img", { name: "Imagem a" })).toHaveAttribute("src", "data:image/png;id=corrected-1");
    expect(screen.getByRole("button", { name: "Imagem a corrigir: rosto 1" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Antes e depois: mostrar original" }));
    expect(screen.getByRole("button", { name: "Imagem a corrigir: rosto 1" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Antes e depois: mostrar correção" }));
    expect(vi.mocked(detectFaces)).toHaveBeenCalledTimes(2);
    fireEvent.click(screen.getByRole("button", { name: "Imagem a corrigir: rosto 1" }));
    await waitFor(() => expect(onCorrection.mock.calls.filter(([action]) => action.kind === "preview")).toHaveLength(3));
    fireEvent.click(screen.getByRole("button", { name: "Imagem a corrigir: rosto 2" }));
    await waitFor(() => expect(onCorrection.mock.calls.filter(([action]) => action.kind === "preview")).toHaveLength(4));
    expect(onCorrection.mock.calls[onCorrection.mock.calls.length - 1]?.[0]).toEqual(expect.objectContaining({ targetFace: second, referenceFace: first }));
    expect(screen.queryByRole("button", { name: "Salvar correção" })).not.toBeInTheDocument();
    view.rerender(<ImageViewer presentation={{ ...initial, correction: { ...correction, phase: "processing", resultUrl: "data:image/png;id=corrected-1" } }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
    expect(screen.getByRole("img", { name: "Imagem a" })).toHaveAttribute("src", initial.url);
    expect(screen.getByRole("button", { name: "Imagem a corrigir: rosto 2" })).toHaveAttribute("aria-pressed", "true");
    view.rerender(<ImageViewer presentation={{ ...initial, correction: { ...correction, phase: "preview", resultUrl: "data:image/png;id=corrected-2" } }} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
    expect(screen.getByRole("img", { name: "Imagem a" })).toHaveAttribute("src", "data:image/png;id=corrected-2");
    fireEvent.click(screen.getByRole("button", { name: "Referência: rosto 2" }));
    await waitFor(() => expect(onCorrection.mock.calls.filter(([action]) => action.kind === "preview")).toHaveLength(5));
    expect(onCorrection.mock.calls[onCorrection.mock.calls.length - 1]?.[0]).toEqual(expect.objectContaining({ targetFace: second, referenceFace: second }));
    expect(vi.mocked(detectFaces)).toHaveBeenCalledTimes(2);
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("each photo owns its no-face tooltip, which closes when focus leaves", async () => {
  vi.mocked(detectFaces).mockResolvedValue([]);
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "select", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  } };
  const view = render(<ImageViewer presentation={presentation} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={vi.fn()} />);
  const reference = screen.getByRole("img", { name: "Referência.jpg" });
  const target = screen.getByRole("img", { name: "Imagem a" });
  for (const photo of [reference, target]) {
    Object.defineProperties(photo, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
    fireEvent.load(photo);
  }
  await waitFor(() => expect(view.container.querySelectorAll("[data-analysis='no-face']")).toHaveLength(2));
  screen.getByRole("group", { name: "Referência: aviso" }).focus();
  expect(await screen.findByRole("tooltip")).toHaveTextContent("Nenhum rosto encontrado na referência.");
  screen.getByRole("group", { name: "Referência: aviso" }).blur();
  await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());
  screen.getByRole("group", { name: "Imagem a corrigir: aviso" }).focus();
  expect(await screen.findByRole("tooltip")).toHaveTextContent("Nenhum rosto encontrado na foto de destino.");
});

test("failed reference analysis explains why preview is unavailable on the reference photo", async () => {
  const face = [{ x: .3, y: .2, z: 0 }, { x: .7, y: .7, z: 0 }];
  vi.mocked(detectFaces).mockImplementation((image) => image.alt === "Referência.jpg" ? Promise.reject(new Error("analysis failed")) : Promise.resolve([face]));
  const onCorrection = vi.fn();
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "select", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error: null,
  } };
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(500);
  try {
    const view = render(<ImageViewer presentation={presentation} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
    for (const image of screen.getAllByRole("img")) {
      Object.defineProperties(image, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
      fireEvent.load(image);
    }
    await waitFor(() => expect(view.container.querySelector(".eye-correction__pane:first-child [data-analysis='failed']")).toBeInTheDocument());
    fireEvent.click(await screen.findByRole("button", { name: "Imagem a corrigir: rosto 1" }));
    expect(screen.queryByRole("button", { name: "Ver correção" })).not.toBeInTheDocument();
    screen.getByRole("group", { name: "Referência: aviso" }).focus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Não foi possível analisar a referência.");
    expect(onCorrection).not.toHaveBeenCalled();
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("preparation errors appear on the destination photo and keep preview retry available", async () => {
  const face = [{ x: .3, y: .2, z: 0 }, { x: .7, y: .7, z: 0 }];
  vi.mocked(detectFaces).mockResolvedValue([face]);
  const error = "Não foi possível preparar a correção para esta fotografia. Escolha outra referência e tente novamente.";
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "select", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: null, error,
  } };
  const onCorrection = vi.fn();
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(600);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(500);
  try {
    const view = render(<ImageViewer presentation={presentation} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
    for (const image of screen.getAllByRole("img")) {
      Object.defineProperties(image, { naturalWidth: { configurable: true, value: 800 }, naturalHeight: { configurable: true, value: 1200 } });
      fireEvent.load(image);
    }
    fireEvent.click(await screen.findByRole("button", { name: "Imagem a corrigir: rosto 1" }));
    fireEvent.click(screen.getByRole("button", { name: "Referência: rosto 1" }));
    await waitFor(() => expect(onCorrection.mock.calls.filter(([action]) => action.kind === "preview")).toHaveLength(1));
    expect(view.container.querySelector(".eye-correction__hint")).not.toBeInTheDocument();
    screen.getByRole("group", { name: "Imagem a corrigir: aviso" }).focus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent(error);
    screen.getByRole("button", { name: "Imagem a corrigir: rosto 1" }).focus();
    await waitFor(() => expect(screen.getByRole("tooltip")).toHaveTextContent(error));
    fireEvent.click(screen.getByRole("button", { name: "Referência: rosto 1" }));
    await waitFor(() => expect(onCorrection.mock.calls.filter(([action]) => action.kind === "preview")).toHaveLength(2));
  } finally { width.mockRestore(); height.mockRestore(); }
});

test("save errors appear on the destination photo and still permit one retry", async () => {
  const error = "Não foi possível salvar a cópia corrigida.";
  const onCorrection = vi.fn();
  const presentation: ViewerPresentation = { ...initial, correction: {
    phase: "preview", referenceMediaId: "reference", referenceName: "Referência.jpg", referenceUrl: "data:image/png;id=reference",
    referenceState: "ready", canPreviousReference: false, canNextReference: false, resultUrl: "data:image/png;id=corrected", error,
  } };
  render(<ImageViewer presentation={presentation} onNavigate={vi.fn()} onClose={vi.fn()} onCorrection={onCorrection} />);
  const retry = screen.getByRole("button", { name: "Salvar correção" });
  expect(retry).toBeEnabled();
  screen.getByRole("group", { name: "Imagem a corrigir: aviso" }).focus();
  expect(await screen.findByRole("tooltip")).toHaveTextContent(error);
  retry.focus();
  await waitFor(() => expect(screen.getByRole("tooltip")).toHaveTextContent("Salvar correção"));
  expect(screen.getByRole("tooltip")).not.toHaveTextContent(error);
  fireEvent.click(retry);
  fireEvent.click(retry);
  expect(onCorrection).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Substituir original" }));
  expect(onCorrection.mock.calls.filter(([action]) => action.kind === "apply")).toHaveLength(1);
});
