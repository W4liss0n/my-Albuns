import { rasterLimitsAt300Dpi } from "../test/projectConfigurationFixtures";
import { fireEvent, render, screen, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { beforeEach, expect, test, vi } from "vitest";

import { representativeProjection } from "../test/projectFixtures";
import { frameStyleCorpus } from "../test/frameStylePreview";
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
    mediaItems: representativeProjection.state.album.media,
    mediaPreviews: {},
    onApplyAlbumDesign: vi.fn(),
    onApplyAlbumInformation: vi.fn(),    onNavigateToSheet: vi.fn(),
    onPresentationUnitChange: vi.fn(),    onValidateAlbumInformation: vi.fn(async () => ({ rasterLimits: rasterLimitsAt300Dpi,
      errors: [],
      impact: { conversionLosses: [],
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
    frameGapUm: 5_000,  };
}

function sheetContext(): InspectorContext {
  return { kind: "sheet", sheet: composedSheet };
}

beforeEach(() => localStorage.clear());

test("group Photo Zoom stays mixed until an explicit value and ignores placeholders", () => {
  const frames = [structuredClone(sheetState.frames[0]), { ...structuredClone(sheetState.frames[0]), id: "second-photo" }];
  frames[0].photo!.transform.userZoom = 1.5;
  frames[1].photo!.transform.userZoom = 2;
  frames.push({ ...structuredClone(frames[0]), id: "placeholder", photo: null });
  const onCommit = vi.fn();
  const actions = { disabled: false, scopeKey: "zoom", doubleClickTimeMs: 500,
    dragThreshold: { x: 5, y: 5 }, onPreview: vi.fn(), onCommit, onCancel: vi.fn() };
  const props = inspectorProps({ kind: "multiple-frames", frames, editingSheet: composedSheet });
  const view = render(<InspectorPanel {...props} photoZoom={actions} />);
  const field = screen.getByRole("spinbutton", { name: "Zoom da foto em porcentagem" });
  expect(field).toHaveValue("");
  expect(field).toHaveAttribute("aria-valuetext", "Múltiplos valores");
  expect(onCommit).not.toHaveBeenCalled();
  fireEvent.change(field, { target: { value: "175" } });
  fireEvent.keyDown(field, { key: "Enter" });
  expect(onCommit).toHaveBeenCalledExactlyOnceWith(175);
  view.rerender(<InspectorPanel {...props} context={{ kind: "multiple-frames",
    frames: frames.map((frame) => ({ ...frame, photo: null })), editingSheet: composedSheet,
  }} photoZoom={actions} />);
  expect(screen.queryByRole("slider", { name: "Zoom da foto" })).not.toBeInTheDocument();
});

test("Frame style keeps mixed values neutral and lets an explicit color or restoration affect the selection", () => {
  const state = frameStyleCorpus.states["single-custom"];
  const frames = state.state.album.sheets[0].frames;
  const onCommit = vi.fn();
  const actions = { disabled: false, scopeKey: "styles", doubleClickTimeMs: 500,
    dragThreshold: { x: 5, y: 5 }, onPreview: vi.fn(), onCommit, onCancel: vi.fn() };
  const props = inspectorProps({ kind: "multiple-frames", frames, editingSheet: state.composition.sheets[0] });
  const view = render(<InspectorPanel {...props} frameStyle={actions} />);
  expect(screen.getByRole("spinbutton", { name: "Opacidade em porcentagem" })).toHaveValue("");
  expect(screen.getByRole("spinbutton", { name: "Espessura da borda em mm" })).toHaveValue("");
  const color = screen.getByRole("button", { name: "Cor da borda" });
  expect(color).toHaveAttribute("data-mixed", "true");
  expect(onCommit).not.toHaveBeenCalled();
  fireEvent.click(color);
  fireEvent.click(screen.getByRole("button", { name: "Aplicar cor" }));
  expect(onCommit).toHaveBeenCalledExactlyOnceWith({ kind: "borderColor", rgb: "#000000" });
  fireEvent.click(screen.getByRole("button", { name: "Usar padrão do álbum" }));
  expect(onCommit).toHaveBeenLastCalledWith({ kind: "restoreAlbum" });
  view.rerender(<InspectorPanel {...props} context={{ kind: "multiple-frames",
    frames: frames.map((frame) => ({ ...frame, photo: null })), editingSheet: state.composition.sheets[0],
  }} frameStyle={actions} />);
  expect(screen.getByRole("slider", { name: "Opacidade do quadro" })).toBeEnabled();
  expect(screen.queryByRole("button", { name: "Ajustes e Efeitos" })).not.toBeInTheDocument();
});

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
  expect(screen.getByText("Aplicado a 2 fotos de 3 quadros")).toBeInTheDocument();
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
  expect(screen.getByText("Aplicado a 2 fotos de 3 quadros")).toBeInTheDocument();
  expect(screen.getByLabelText("Giro das fotos")).toHaveTextContent("—");
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
    screen.queryByRole("button", { name: "Informações do álbum" }),
  ).not.toBeInTheDocument();
  const sectionTrigger = screen.getByRole("button", {
    name: "Design da lâmina",
  });
  expect(sectionTrigger).toHaveAttribute("aria-expanded", "true");
  expect(
    screen.getByRole("img", { name: "Prévia da lâmina 01" }),
  ).toBeInTheDocument();

  const both = screen.getByRole("button", { name: "Ambos os lados" });
  const left = screen.getByRole("button", { name: "Página esquerda" });
  expect(both).toHaveAttribute("aria-pressed", "true");

  const preview = left.closest('[role="group"]') as HTMLElement;
  fireEvent.pointerEnter(left);
  expect(preview).toHaveAttribute("data-hovered-scope", "left");
  expect(both).toHaveAttribute("aria-pressed", "true");
  fireEvent.pointerLeave(preview);

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
  expect(screen.getByText("Quadro selecionado")).toBeInTheDocument();

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
    name: "Aplicar na lâmina 01",
  });
  expect(preview).toHaveAttribute("data-active-sides", "right");
  expect(preview).toHaveStyle({
    "--sheet-design-aspect-ratio": "600000 / 300000",
  });
});

test("uses the shared decorative picker fallback when a Sheet decorative has no preview", () => {
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
      mediaItems={[{ id: "decorative-without-preview", kind: "decorative", name: "Textura sem prévia", palette: null, sourceWidthPx: null, sourceHeightPx: null }]}
    />,
  );

  const value = screen.getByRole("button", { name: "Decorativo do fundo: Textura sem prévia. Escolher outro" });
  expect(value.querySelector(".visual-design-picker__tile")).toHaveStyle(
    { background: "var(--ui-surface-muted)" },
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

  const trigger = screen.getByRole("button", { name: "Design da lâmina" });
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

test("shows Sheet-design origin and local controls without placeholders", () => {
  render(<InspectorPanel {...inspectorProps(sheetContext())} />);

  const section = screen
    .getByRole("button", { name: "Design da lâmina" })
    .closest("section") as HTMLElement;
  const design = within(section);

  expect(design.getByText("Fundo")).toBeInTheDocument();
  expect(design.getByText("Sobreposição")).toBeInTheDocument();
  expect(design.getByRole("group", { name: "Opções de fundo" })).toHaveAccessibleDescription(/Usando o padrão do álbum/);
  expect(design.getByRole("group", { name: "Opções de sobreposição" })).toHaveAccessibleDescription(/Usando o padrão do álbum/);
  expect(design.getByRole("button", { name: "Cor do fundo da lâmina" })).toBeInTheDocument();
  expect(design.queryByText("Origem ainda não disponível")).not.toBeInTheDocument();
  expect(section.querySelector('[data-placeholder-feature="edit-sheet-background"]')).toBeNull();
  expect(section.querySelector('[data-placeholder-feature="edit-sheet-overlay"]')).toBeNull();
  expect(design.getByRole("button", { name: "Salvar disposição como layout" })).toBeDisabled();
  expect(section.querySelector('[data-placeholder-feature="save-sheet-layout"]')).toBeNull();
});
