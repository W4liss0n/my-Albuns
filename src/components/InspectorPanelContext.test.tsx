import { fireEvent, render, screen, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { beforeEach, expect, test, vi } from "vitest";

import { representativeProjection } from "../test/projectFixtures";
import {
  InspectorPanel,
  type InspectorContext,
} from "./InspectorPanel";

const composedSheet = representativeProjection.composition.sheets[0];
const sheetState = representativeProjection.state.album.sheets[0];

function inspectorProps(
  context: InspectorContext,
): ComponentProps<typeof InspectorPanel> {
  return {
    context,
    displayedPhotoPanX: 0,
    displayedPhotoZoom: 1,
    document: representativeProjection.state.document,
    focusedSheetId: composedSheet.sheetId,
    frameBorder: representativeProjection.composition.frameBorder,
    mediaItems: representativeProjection.state.album.media,
    mediaPreviewUrls: {},
    onApplyAlbumDesign: vi.fn(),
    onApplyAlbumInformation: vi.fn(),
    onBeginPhotoZoom: vi.fn(),
    onFinishPhotoZoom: vi.fn(),
    onNavigateToSheet: vi.fn(),
    onPresentationUnitChange: vi.fn(),
    onUpdatePhotoZoom: vi.fn(),
    onValidateAlbumInformation: vi.fn(async () => ({
      errors: [],
      impact: {
        heightPx: 3_543,
        pageWidthPx: 3_543,
        sheetWidthPx: 7_087,
      },
    })),
    presentationUnit: representativeProjection.state.document.displayUnit,
    sheets: representativeProjection.composition.sheets,
    sheetStates: representativeProjection.state.album.sheets,
    visualDefaults: representativeProjection.state.album.visualDefaults,
    zoomCommitting: false,
  };
}

function sheetContext(): InspectorContext {
  return { kind: "sheet", sheet: composedSheet };
}

beforeEach(() => localStorage.clear());

test("shows Design da Lâmina and preserves its scope while Frame temporarily owns the Inspector", () => {
  const view = render(<InspectorPanel {...inspectorProps(sheetContext())} />);

  expect(
    screen.queryByRole("button", { name: "Informações do Álbum" }),
  ).not.toBeInTheDocument();
  const sectionTrigger = screen.getByRole("button", {
    name: "Design da Lâmina",
  });
  expect(sectionTrigger).toHaveAttribute("aria-expanded", "true");
  expect(
    screen.getByRole("img", { name: "Prévia da Lâmina 01" }),
  ).toBeInTheDocument();

  const both = screen.getByRole("button", { name: "Ambos os lados" });
  const left = screen.getByRole("button", { name: "Página esquerda" });
  expect(both).toHaveAttribute("aria-pressed", "true");

  const preview = left.closest('[role="group"]') as HTMLElement;
  fireEvent.mouseEnter(left);
  expect(preview).toHaveAttribute("data-hovered-scope", "left");
  expect(both).toHaveAttribute("aria-pressed", "true");
  fireEvent.mouseLeave(preview);

  fireEvent.click(left);
  expect(left).toHaveAttribute("aria-pressed", "true");

  view.rerender(
    <InspectorPanel
      {...inspectorProps({
        kind: "frame",
        frame: sheetState.frames[0],
        composedPhoto: composedSheet.frames[0].photo,
        editingSheet: composedSheet,
      })}
    />,
  );
  expect(screen.getByText("Frame selecionado")).toBeInTheDocument();

  view.rerender(<InspectorPanel {...inspectorProps(sheetContext())} />);
  expect(
    screen.getByRole("button", { name: "Página esquerda" }),
  ).toHaveAttribute("aria-pressed", "true");

  view.rerender(
    <InspectorPanel
      {...inspectorProps({
        kind: "frame",
        frame: sheetState.frames[0],
        composedPhoto: composedSheet.frames[0].photo,
      })}
    />,
  );
  view.rerender(<InspectorPanel {...inspectorProps(sheetContext())} />);
  expect(
    screen.getByRole("button", { name: "Ambos os lados" }),
  ).toHaveAttribute("aria-pressed", "true");
});

test("keeps inactive sides inert for a single-page Sheet", () => {
  const singleSheet = {
    ...composedSheet,
    activeSides: "right" as const,
    widthUm: 300_000,
  };
  render(
    <InspectorPanel
      {...inspectorProps({
        kind: "sheet",
        sheet: singleSheet,
      })}
    />,
  );

  expect(
    screen.getByRole("button", { name: "Página direita" }),
  ).toHaveAttribute("aria-pressed", "true");
  expect(
    screen.queryByRole("button", { name: "Página esquerda" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Ambos os lados" }),
  ).not.toBeInTheDocument();
  expect(
    document.querySelector(
      '.sheet-design-preview__inactive[data-side="left"]',
    ),
  ).toBeInTheDocument();
  const preview = screen.getByRole("group", {
    name: "Selecionar escopo da Lâmina 01",
  });
  expect(preview).toHaveAttribute("data-active-sides", "right");
  expect(preview).toHaveStyle({
    "--sheet-design-aspect-ratio": "600000 / 300000",
  });
});

test("stores accordion preferences independently for the Sheet context", () => {
  render(<InspectorPanel {...inspectorProps(sheetContext())} />);

  const trigger = screen.getByRole("button", { name: "Design da Lâmina" });
  fireEvent.click(trigger);

  expect(localStorage.getItem("myalbuns.inspector.sheet.design")).toBe(
    "closed",
  );
  expect(localStorage.getItem("myalbuns.inspector.album.design")).toBeNull();
  expect(localStorage.getItem("myalbuns.inspector.frame-photo.design")).toBeNull();
});

test("marks unavailable Sheet-design mutations as explicit placeholders", () => {
  render(<InspectorPanel {...inspectorProps(sheetContext())} />);

  const section = screen
    .getByRole("button", { name: "Design da Lâmina" })
    .closest("section") as HTMLElement;
  const design = within(section);

  expect(design.getByText("Background")).toBeInTheDocument();
  expect(design.getByText("Overlay")).toBeInTheDocument();
  expect(
    section.querySelector('[data-placeholder-feature="edit-sheet-background"]'),
  ).toBeDisabled();
  expect(
    section.querySelector('[data-placeholder-feature="edit-sheet-overlay"]'),
  ).toBeDisabled();
  expect(
    section.querySelector('[data-placeholder-feature="save-sheet-layout"]'),
  ).toBeDisabled();
});
