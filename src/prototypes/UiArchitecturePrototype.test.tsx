import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test } from "vitest";

import { UiArchitecturePrototype } from "./UiArchitecturePrototype";

test("exposes every canonical surface as a navigable, stable map node", () => {
  render(<UiArchitecturePrototype />);

  const expectedSurfaceIds = [
    "global.welcome",
    "global.new-project.configuration",
    "global.new-project.personalization",
    "native.project-name-location",
    "project.normal",
    "project.edit",
    "project.export",
    "global.batch-export",
    "project.batch-generation",
    "shared.problems",
    "shared.progress",
    "global.settings",
  ];
  const nodes = screen.getAllByTestId("surface-map-node");

  expect(nodes.map((node) => node.dataset.surfaceId)).toEqual(
    expectedSurfaceIds,
  );
  for (const node of nodes) {
    const link = within(node).getByRole("link");
    expect(link.getAttribute("href")).toMatch(/^\//u);
    expect(within(node).getByText(/Owner:/u)).toBeInTheDocument();
  }
  expect(screen.getByText("Configurações", { selector: "strong" })).toBeInTheDocument();
  expect(screen.queryByText("Dimensões", { selector: "strong" })).not.toBeInTheDocument();
  expect(screen.getByTestId("surface-transition-map")).toHaveTextContent(
    "Boas-vindas → Configurações → Personalização → Nome e local → Projeto",
  );
});

test("applies only Ctrl zoom gestures between Ajustar Lâmina and the calibrated 4× cap", () => {
  render(<UiArchitecturePrototype initialView="editor" />);

  const canvas = screen.getByRole("region", { name: "Canvas do protótipo" });
  expect(canvas).toHaveAttribute("data-zoom-level", "1");

  fireEvent.keyDown(canvas, { ctrlKey: true, key: "+" });
  expect(canvas).toHaveAttribute("data-zoom-level", "1.25");
  expect(canvas).toHaveAttribute("data-last-zoom-input", "keyboard-in");

  fireEvent.keyDown(canvas, { ctrlKey: true, key: "-" });
  expect(canvas).toHaveAttribute("data-zoom-level", "1");

  fireEvent.wheel(canvas, { ctrlKey: true, deltaY: -120 });
  expect(canvas).toHaveAttribute("data-zoom-level", "1.25");
  expect(canvas).toHaveAttribute("data-last-zoom-input", "wheel-in");
  expect(canvas).toHaveAttribute("data-zoom-anchor", "cursor");

  fireEvent.wheel(canvas, { deltaY: -120 });
  expect(canvas).toHaveAttribute("data-zoom-level", "1.25");

  for (let step = 0; step < 20; step += 1) {
    fireEvent.keyDown(canvas, { ctrlKey: true, key: "+" });
  }
  expect(canvas).toHaveAttribute("data-zoom-level", "4");

  fireEvent.keyDown(canvas, { ctrlKey: true, key: "0" });
  expect(canvas).toHaveAttribute("data-zoom-level", "1");
  expect(canvas).toHaveAttribute("data-last-zoom-input", "reset");
  expect(canvas).toHaveAttribute("data-zoom-state", "fit");
  expect(screen.queryByText(/\d+%/u)).not.toBeInTheDocument();
  expect(screen.queryByText(/Ctrl\+/u)).not.toBeInTheDocument();
});

test("previews, cancels, and commits one synchronized reorder from the Barra", () => {
  render(<UiArchitecturePrototype initialView="editor" />);

  const bar = screen.getByRole("region", { name: "Barra de Lâminas" });
  const grid = screen.getByRole("region", { name: "Grade de Lâminas" });
  const originalOrder = "sheet-001,sheet-002,sheet-003,sheet-004,sheet-005";
  const reordered = "sheet-001,sheet-004,sheet-002,sheet-003,sheet-005";

  expect(bar).toHaveAttribute("data-sheet-order", originalOrder);
  expect(grid).toHaveAttribute("data-sheet-order", originalOrder);

  const source = within(bar).getByRole("button", {
    name: "Reordenar Lâmina 04 pela Barra",
  });
  const target = within(bar).getByRole("button", {
    name: "Reordenar Lâmina 02 pela Barra",
  });
  fireEvent.pointerDown(source, { clientX: 10, clientY: 10, pointerId: 1 });
  fireEvent.pointerMove(target, { clientX: 40, clientY: 40, pointerId: 1 });

  expect(bar).toHaveAttribute("data-reorder-state", "preview");
  expect(bar).toHaveAttribute("data-preview-order", reordered);
  expect(bar).toHaveAttribute("data-sheet-order", originalOrder);
  expect(grid).toHaveAttribute("data-sheet-order", originalOrder);
  expect(within(bar).getByTestId("reorder-placeholder")).toBeInTheDocument();
  expect(within(bar).getByTestId("reorder-ghost")).toBeInTheDocument();

  fireEvent.keyDown(document, { key: "Escape" });
  expect(bar).toHaveAttribute("data-reorder-state", "cancelled");
  expect(bar).toHaveAttribute("data-sheet-order", originalOrder);
  expect(within(bar).queryByTestId("reorder-placeholder")).not.toBeInTheDocument();

  fireEvent.pointerDown(source, { clientX: 10, clientY: 10, pointerId: 2 });
  fireEvent.pointerMove(target, { clientX: 40, clientY: 40, pointerId: 2 });
  fireEvent.pointerUp(bar, { clientX: 40, clientY: 40, pointerId: 2 });

  expect(bar).toHaveAttribute("data-reorder-state", "committed");
  expect(bar).toHaveAttribute("data-sheet-order", reordered);
  expect(grid).toHaveAttribute("data-sheet-order", reordered);
  expect(screen.getByTestId("prototype-history-count")).toHaveTextContent("1");
});

test("keeps the Canvas stable during a Grade preview and rejects an interior Página única drop", () => {
  render(<UiArchitecturePrototype initialView="editor" />);

  const bar = screen.getByRole("region", { name: "Barra de Lâminas" });
  const grid = screen.getByRole("region", { name: "Grade de Lâminas" });
  const originalOrder = "sheet-001,sheet-002,sheet-003,sheet-004,sheet-005";
  const source = within(grid).getByRole("button", {
    name: "Reordenar Lâmina 01 pela Grade",
  });
  const invalidTarget = within(grid).getByRole("button", {
    name: "Reordenar Lâmina 03 pela Grade",
  });

  fireEvent.pointerDown(source, { clientX: 10, clientY: 10, pointerId: 3 });
  fireEvent.pointerMove(invalidTarget, {
    clientX: 80,
    clientY: 80,
    pointerId: 3,
  });

  expect(grid).toHaveAttribute("data-reorder-state", "invalid-target");
  expect(grid).toHaveAttribute("data-sheet-order", originalOrder);
  expect(bar).toHaveAttribute("data-sheet-order", originalOrder);
  expect(within(grid).queryByTestId("reorder-placeholder")).not.toBeInTheDocument();

  fireEvent.pointerUp(invalidTarget, {
    clientX: 80,
    clientY: 80,
    pointerId: 3,
  });
  expect(grid).toHaveAttribute("data-reorder-state", "invalid");
  expect(grid).toHaveAttribute("data-sheet-order", originalOrder);
  expect(screen.getByTestId("prototype-history-count")).toHaveTextContent("0");
});

test("projects mixed values for Ctrl multi-selection and applies the first absolute edit to every Frame", () => {
  render(<UiArchitecturePrototype initialView="editor" />);

  const firstFrame = screen.getByRole("button", { name: "Selecionar Frame 01" });
  const secondFrame = screen.getByRole("button", { name: "Selecionar Frame 02" });
  fireEvent.pointerDown(firstFrame, { button: 0, pointerId: 10 });
  fireEvent.pointerUp(firstFrame, { button: 0, pointerId: 10 });
  fireEvent.click(firstFrame);
  fireEvent.pointerDown(secondFrame, {
    button: 0,
    ctrlKey: true,
    pointerId: 11,
  });
  fireEvent.pointerUp(secondFrame, {
    button: 0,
    ctrlKey: true,
    pointerId: 11,
  });
  fireEvent.click(secondFrame, { ctrlKey: true });

  expect(firstFrame).toHaveAttribute("aria-pressed", "true");
  expect(secondFrame).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByTestId("frame-selection-bounds")).toBeInTheDocument();

  const inspector = screen.getByRole("region", { name: "Inspector de Frames" });
  expect(inspector).toHaveAttribute("data-selection-count", "2");
  expect(within(inspector).getByText("2 Frames · 1 Foto · 1 placeholder")).toBeInTheDocument();
  const opacity = within(inspector).getByRole("spinbutton", {
    name: "Opacidade dos Frames",
  });
  expect(opacity).toHaveValue(null);
  expect(opacity).toHaveAttribute("placeholder", "—");
  expect(opacity).toHaveAttribute("data-mixed-value", "numeric");
  expect(within(inspector).getByTestId("mixed-color")).toHaveAttribute(
    "data-mixed-value",
    "color",
  );
  expect(within(inspector).getByRole("checkbox", { name: "Borda dos Frames" })).toHaveAttribute(
    "aria-checked",
    "mixed",
  );

  fireEvent.change(opacity, { target: { value: "80" } });

  expect(opacity).toHaveValue(80);
  expect(opacity).not.toHaveAttribute("data-mixed-value");
  expect(firstFrame).toHaveAttribute("data-opacity", "80");
  expect(secondFrame).toHaveAttribute("data-opacity", "80");
  expect(screen.getByTestId("prototype-history-count")).toHaveTextContent("1");
});

test("moves and resizes one selected Frame, then blocks both gestures when Layout is locked", () => {
  render(<UiArchitecturePrototype initialView="editor" />);

  const frame = screen.getByRole("button", { name: "Selecionar Frame 01" });
  fireEvent.click(frame);
  expect(screen.getAllByTestId("frame-resize-handle")).toHaveLength(8);

  const moveTarget = screen.getByTestId("frame-move-target");
  fireEvent.pointerDown(frame, { button: 0, clientX: 10, clientY: 10, pointerId: 4 });
  fireEvent.pointerMove(moveTarget, { clientX: 90, clientY: 70, pointerId: 4 });
  expect(frame).toHaveAttribute("data-x", "16");
  expect(frame).toHaveAttribute("data-y", "18");
  expect(screen.getByTestId("prototype-history-count")).toHaveTextContent("0");
  fireEvent.pointerUp(moveTarget, { clientX: 90, clientY: 70, pointerId: 4 });
  expect(screen.getByTestId("prototype-history-count")).toHaveTextContent("1");

  const southeastHandle = screen.getByRole("button", {
    name: "Redimensionar Frame 01 pelo canto inferior direito",
  });
  const resizeTarget = screen.getByTestId("frame-resize-target");
  fireEvent.pointerDown(southeastHandle, {
    button: 0,
    clientX: 90,
    clientY: 70,
    pointerId: 5,
  });
  fireEvent.pointerMove(resizeTarget, {
    clientX: 120,
    clientY: 100,
    pointerId: 5,
  });
  fireEvent.pointerUp(resizeTarget, {
    clientX: 120,
    clientY: 100,
    pointerId: 5,
  });
  expect(frame).toHaveAttribute("data-width", "38");
  expect(frame).toHaveAttribute("data-height", "42");
  expect(screen.getByTestId("prototype-history-count")).toHaveTextContent("2");

  fireEvent.click(screen.getByRole("button", { name: "Layout travado" }));
  expect(frame).toHaveAttribute("aria-pressed", "true");
  expect(screen.queryAllByTestId("frame-resize-handle")).toHaveLength(0);
  fireEvent.pointerDown(frame, { button: 0, pointerId: 6 });
  fireEvent.pointerMove(moveTarget, { pointerId: 6 });
  fireEvent.pointerUp(moveTarget, { pointerId: 6 });
  expect(frame).toHaveAttribute("data-x", "16");
  expect(frame).toHaveAttribute("data-y", "18");
  expect(screen.getByTestId("layout-lock-feedback")).toHaveTextContent(
    "Layout travado: seleção preservada; mover e redimensionar estão bloqueados.",
  );
  expect(screen.getByTestId("layout-lock-feedback")).toHaveAttribute(
    "data-layout-lock-feedback",
  );
  expect(screen.getByTestId("prototype-history-count")).toHaveTextContent("2");
});
