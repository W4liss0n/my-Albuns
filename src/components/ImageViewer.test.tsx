import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { ImageViewer } from "./ImageViewer";
import type { ViewerPresentation } from "../application/imageViewerWindow";

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
