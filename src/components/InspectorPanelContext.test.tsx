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
    mediaPreviews: {},
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
    revision: representativeProjection.state.revision,
    sectionState: { kind: "local" },
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

test("Black and white has its own Effects section and exposes the mixed Photo state", () => {
  const frames = [structuredClone(sheetState.frames[0]), { ...structuredClone(sheetState.frames[0]), id: "second-photo" }];
  frames[0].photo!.transform.blackAndWhite = true;
  frames[1].photo!.transform.blackAndWhite = false;
  frames.push({ ...structuredClone(frames[0]), id: "placeholder", photo: null });
  const onToggleBlackAndWhite = vi.fn();
  const photoEffects = { disabled: false, onToggleBlackAndWhite };
  const props = inspectorProps({ kind: "multiple-frames", frames, editingSheet: composedSheet });
  const view = render(<InspectorPanel {...props} photoEffects={photoEffects} />);
  const section = screen.getByRole("button", { name: "Ajustes e Efeitos" });
  expect(section).toHaveAttribute("aria-expanded", "true");
  const effect = screen.getByRole("button", { name: "Preto e branco" });
  expect(effect).toHaveAttribute("aria-pressed", "mixed");
  expect(screen.getByText("Aplicado a 2 Fotos de 3 Frames")).toBeInTheDocument();
  fireEvent.click(effect);
  expect(onToggleBlackAndWhite).toHaveBeenCalledOnce();
  fireEvent.click(section);
  expect(screen.queryByRole("button", { name: "Preto e branco" })).not.toBeInTheDocument();
  fireEvent.click(section);
  view.rerender(<InspectorPanel {...props} photoEffects={{ ...photoEffects, disabled: true }} />);
  expect(screen.getByRole("button", { name: "Preto e branco" })).toBeDisabled();
  view.rerender(<InspectorPanel {...inspectorProps({ kind: "frame", frame: frames[2], composedPhoto: null })} photoEffects={photoEffects} />);
  expect(screen.queryByRole("button", { name: "Ajustes e Efeitos" })).not.toBeInTheDocument();
  view.rerender(<InspectorPanel {...inspectorProps({ kind: "frame", frame: frames[0], composedPhoto: composedSheet.frames[0].photo })} photoEffects={photoEffects} />);
  expect(screen.getByRole("button", { name: "Preto e branco" })).toHaveAttribute("aria-pressed", "true");
});

test("Photo orientation controls show mixed values and target compatible Photos", () => {
  const frames = [structuredClone(sheetState.frames[0]), { ...structuredClone(sheetState.frames[0]), id: "second-photo" }];
  frames[0].photo!.transform.quarterTurns = 3;
  frames[0].photo!.transform.mirrorX = true;
  frames[1].photo!.transform.quarterTurns = 0;
  frames[1].photo!.transform.mirrorX = false;
  frames.push({ ...structuredClone(frames[0]), id: "placeholder", photo: null });
  const onAction = vi.fn();
  const props = inspectorProps({ kind: "multiple-frames", frames, editingSheet: composedSheet });
  const view = render(<InspectorPanel {...props} photoOrientation={{ disabled: false, onAction }} />);
  expect(screen.getByText("Aplicado a 2 Fotos de 3 Frames")).toBeInTheDocument();
  expect(screen.getByLabelText("Giro das Fotos")).toHaveTextContent("—");
  const mirror = screen.getByRole("button", { name: "Espelhar horizontalmente" });
  expect(mirror).toHaveAttribute("aria-pressed", "mixed");
  fireEvent.click(screen.getByRole("button", { name: "Girar 90° à esquerda" }));
  fireEvent.click(mirror);
  fireEvent.click(screen.getByRole("button", { name: "Restaurar giro" }));
  expect(onAction.mock.calls).toEqual([["rotateCounterClockwise"], ["toggleHorizontalMirror"], ["resetRotation"]]);
  view.rerender(<InspectorPanel {...props} photoOrientation={{ disabled: true, onAction }} />);
  expect(screen.getByRole("button", { name: "Girar 90° à esquerda" })).toBeDisabled();
  expect(mirror).toBeDisabled();
  view.rerender(<InspectorPanel {...inspectorProps({ kind: "frame", frame: frames[2], composedPhoto: null })}
    photoOrientation={{ disabled: false, onAction }} />);
  expect(screen.queryByRole("button", { name: "Espelhar horizontalmente" })).not.toBeInTheDocument();
});

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

test("uses the canonical media fallback when a Sheet decorative has no preview", () => {
  const sheetWithoutDecorativePreview = {
    ...composedSheet,
    backgrounds: [
      {
        drawRect: {
          height: composedSheet.heightUm,
          width: composedSheet.widthUm,
          x: 0,
          y: 0,
        },
        kind: "media" as const,
        mediaId: "decorative-without-preview",
        name: "Textura sem prévia",
      },
    ],
  };
  render(
    <InspectorPanel
      {...inspectorProps({
        kind: "sheet",
        sheet: sheetWithoutDecorativePreview,
      })}
    />,
  );

  const value = screen.getByText("Textura sem prévia").closest(
    ".sheet-design-value",
  ) as HTMLElement;
  expect(value.querySelector(".sheet-design-value__swatch--media")).toHaveStyle(
    { backgroundColor: "#D8DEE2" },
  );
});

test("reads and publishes accordion preferences through the shared workspace state", () => {
  const onSectionPreferenceChange = vi.fn();
  const view = render(
    <InspectorPanel
      {...inspectorProps(sheetContext())}
      sectionState={{
        kind: "controlled",
        values: { "sheet.design": true },
        onChange: onSectionPreferenceChange,
      }}
    />,
  );

  const trigger = screen.getByRole("button", { name: "Design da Lâmina" });
  fireEvent.click(trigger);

  expect(onSectionPreferenceChange).toHaveBeenCalledWith(
    "sheet.design",
    false,
  );
  expect(localStorage.getItem("myalbuns.inspector.sheet.design")).toBeNull();

  view.rerender(
    <InspectorPanel
      {...inspectorProps(sheetContext())}
      sectionState={{
        kind: "controlled",
        values: { "sheet.design": false },
        onChange: onSectionPreferenceChange,
      }}
    />,
  );
  expect(trigger).toHaveAttribute("aria-expanded", "false");
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
