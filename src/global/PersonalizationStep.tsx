import { useState } from "react";
import { Image as ImageIcon, X } from "lucide-react";

import {
  changeFrameBorderColor as transitionFrameBorderColor,
  changeFrameBorderWidth as transitionFrameBorderWidth,
} from "../application/frameBorderEditor";
import {
  displayUnitLabel,
  formatMicrometers,
} from "../application/physicalMeasurements";
import type {
  ProjectLaunchFailure,
  ProvisionalDecorativeSelectionOutcome,
} from "./application/globalProjectPort";
import {
  type NewProjectDimensionsDraft,
} from "./application/newProjectDimensions";
import {
  clearOverlay,
  fixPersonalizationScope,
  personalizationPreviewFromDraft,
  readBackgroundForFixedScope,
  readOverlayForFixedScope,
  setBackgroundColor,
  setBackgroundImage,
  setOverlayImage,
  type NewProjectPersonalizationDraft,
} from "./application/newProjectPersonalization";
import { ActionButton, AppIcon, FailureNotice } from "../ui";
import { PersonalizationScopeSurface } from "../ui/visualPreview";
import { NewProjectPreviewPanel } from "./NewProjectPreviewPanel";

interface PersonalizationStepProps {
  draft: NewProjectDimensionsDraft;
  failure: ProjectLaunchFailure | null;
  onChange(personalization: NewProjectPersonalizationDraft): void;
  onChooseDecorative(): Promise<ProvisionalDecorativeSelectionOutcome>;
  personalization: NewProjectPersonalizationDraft;
}

export function PersonalizationStep({
  draft,
  failure,
  onChange,
  onChooseDecorative,
  personalization,
}: PersonalizationStepProps) {
  const [pickerFailure, setPickerFailure] =
    useState<ProjectLaunchFailure | null>(null);
  const [focusedScope, setFocusedScope] = useState<
    NewProjectPersonalizationDraft["fixedScope"] | null
  >(null);
  const [hoveredScope, setHoveredScope] = useState<
    NewProjectPersonalizationDraft["fixedScope"] | null
  >(null);
  // PLACEHOLDER UI: o espaço entre Frames ainda não possui contrato de
  // persistência; a medida física controla somente a reprodução desta etapa.
  const [frameGapUm, setFrameGapUm] = useState(6_000);
  const backgroundRead = readBackgroundForFixedScope(personalization);
  const selectedBackground =
    backgroundRead.kind === "uniform" ? backgroundRead.value : null;
  const backgroundColor =
    selectedBackground?.kind === "color" ? selectedBackground.rgb : "#FFFFFF";
  const overlayRead = readOverlayForFixedScope(personalization);
  const selectedOverlay =
    overlayRead.kind === "uniform" ? overlayRead.value : undefined;
  const activeFrameBorder =
    personalization.frameBorder.kind === "solid"
      ? personalization.frameBorder
      : null;
  const frameBorderColor = personalization.frameBorderPreference.rgb;
  const frameBorderWidthUm = activeFrameBorder?.widthUm ?? 0;
  const frameBorderValue = activeFrameBorder
    ? `${formatMicrometers(
        activeFrameBorder.widthUm,
        draft.displayUnit,
      )} ${displayUnitLabel(draft.displayUnit)}`
    : "sem borda";
  const scopeLabel = personalizationScopeLabel(personalization.fixedScope);
  const frameBorderEditor = {
    border: personalization.frameBorder,
    solid: personalization.frameBorderPreference,
  };
  const changeFrameBorderWidth = (widthUm: number) => {
    const next = transitionFrameBorderWidth(frameBorderEditor, widthUm);
    onChange({
      ...personalization,
      frameBorder: next.border,
      frameBorderPreference: next.solid,
    });
  };
  const changeFrameBorderColor = (rgb: string) => {
    const next = transitionFrameBorderColor(frameBorderEditor, rgb);
    onChange({
      ...personalization,
      frameBorder: next.border,
      frameBorderPreference: next.solid,
    });
  };
  const chooseBackground = async () => {
    setPickerFailure(null);
    const outcome = await onChooseDecorative();
    if (outcome.status === "failed") {
      setPickerFailure(outcome.error);
    } else if (outcome.status === "selected") {
      onChange(setBackgroundImage(personalization, outcome.selection));
    }
  };
  const chooseOverlay = async () => {
    setPickerFailure(null);
    const outcome = await onChooseDecorative();
    if (outcome.status === "failed") {
      setPickerFailure(outcome.error);
    } else if (outcome.status === "selected") {
      onChange(setOverlayImage(personalization, outcome.selection));
    }
  };

  return (
    <div className="new-project-content new-project-personalization">
      <NewProjectPreviewPanel
        draft={draft}
        outsideSurfaceAction={{
          label: "Ambos os lados",
          onFocusChange: (focused) =>
            setFocusedScope(focused ? "both" : null),
          onPress: () =>
            onChange(fixPersonalizationScope(personalization, "both")),
          pressed: personalization.fixedScope === "both",
        }}
        surfaceLabel="Prévia do formato da Lâmina"
      >
        {(geometry) => (
          <PersonalizationScopeSurface
            focusedScope={focusedScope}
            frameGapUm={frameGapUm}
            geometry={geometry}
            hoveredScope={hoveredScope}
            personalization={personalizationPreviewFromDraft(personalization)}
            presentation={NEW_PROJECT_SCOPE_PRESENTATION}
            onFocusedScopeChange={setFocusedScope}
            onHoveredScopeChange={setHoveredScope}
            onScopeChange={(scope) =>
              onChange(fixPersonalizationScope(personalization, scope))
            }
          />
        )}
      </NewProjectPreviewPanel>
      <div className="new-project-visual-values">
        <p className="ui-section-eyebrow new-project-scope-label">
          {scopeLabel}
        </p>
        <section className="new-project-value-group">
          <h2>Background</h2>
          <div
            aria-label="Cores de Background"
            className="new-project-color-swatches"
            role="group"
          >
            {BACKGROUND_SWATCHES.map((color) => (
              <button
                aria-label={`Usar Background ${color}`}
                aria-pressed={
                  selectedBackground?.kind === "color" &&
                  selectedBackground.rgb.toLowerCase() === color.toLowerCase()
                }
                key={color}
                onClick={() =>
                  onChange(setBackgroundColor(personalization, color))
                }
                style={{ background: color }}
                type="button"
              />
            ))}
            <label className="new-project-color-picker">
              <span className="ui-visually-hidden">Cor do Background</span>
              <input
                aria-label="Cor do Background"
                onChange={(event) =>
                  onChange(
                    setBackgroundColor(personalization, event.target.value),
                  )
                }
                type="color"
                value={backgroundColor}
              />
            </label>
          </div>
          <ActionButton
            aria-label="Escolher imagem de Background"
            className="new-project-image-action"
            onClick={() => void chooseBackground()}
          >
            <AppIcon icon={ImageIcon} size={14} />
            Usar imagem…
          </ActionButton>
          {backgroundRead.kind === "mixed" ? (
            <p className="new-project-native-note">Valores diferentes</p>
          ) : selectedBackground?.kind === "image" ? (
            <p className="new-project-selection-name">
              {selectedBackground.selection.displayName}
            </p>
          ) : null}
        </section>
        <section className="new-project-value-group">
          <h2>Overlay</h2>
          <ActionButton
            aria-label="Escolher imagem de Overlay"
            className="new-project-image-action new-project-image-action--dashed"
            onClick={() => void chooseOverlay()}
          >
            <AppIcon icon={ImageIcon} size={14} />
            Escolher imagem…
          </ActionButton>
          {overlayRead.kind === "mixed" ? (
            <p className="new-project-native-note">Valores diferentes</p>
          ) : selectedOverlay ? (
            <>
              <p className="new-project-selection-name">
                {selectedOverlay.selection.displayName}
              </p>
              <ActionButton
                aria-label="Remover Overlay"
                density="compact"
                onClick={() => onChange(clearOverlay(personalization))}
                variant="quiet"
              >
                <AppIcon icon={X} size={12} />
                Remover
              </ActionButton>
            </>
          ) : (
            <p className="new-project-native-note">Sem Overlay</p>
          )}
        </section>
        <section className="new-project-value-group">
          <p className="ui-section-eyebrow new-project-group-eyebrow">
            Todas as Lâminas
          </p>
          <h2>Frames</h2>
          <FrameRangeControl
            label="Espessura da Borda padrão"
            max={5_000}
            min={0}
            onChange={changeFrameBorderWidth}
            step={250}
            value={frameBorderWidthUm}
            valueText={frameBorderValue}
            visibleLabel="Borda padrão"
          />
          <div
            aria-label="Cores da Borda"
            className="new-project-color-swatches new-project-color-swatches--frames"
            role="group"
          >
            {FRAME_BORDER_SWATCHES.map((color) => (
              <button
                aria-label={`Usar cor da Borda ${color}`}
                aria-pressed={
                  frameBorderColor.toLowerCase() === color.toLowerCase()
                }
                key={color}
                onClick={() => changeFrameBorderColor(color)}
                style={{ background: color }}
                type="button"
              />
            ))}
          </div>
          <FrameRangeControl
            dataPlaceholderFeature="new-project-frame-gap"
            label="Espaço entre Frames"
            max={24_000}
            min={0}
            onChange={setFrameGapUm}
            step={1_000}
            value={frameGapUm}
            valueText={`${formatMicrometers(
              frameGapUm,
              draft.displayUnit,
            )} ${displayUnitLabel(draft.displayUnit)}`}
            visibleLabel="Espaço entre Frames"
          />
        </section>
        <p className="new-project-native-note">
          Nome e Localização serão escolhidos no diálogo do Windows ao criar.
        </p>
        {pickerFailure ? (
          <FailureNotice
            failure={pickerFailure}
            title="Não foi possível escolher a Imagem decorativa"
          />
        ) : null}
        {failure ? (
          <FailureNotice
            failure={failure}
            title="Não foi possível criar o Projeto"
          />
        ) : null}
      </div>
    </div>
  );
}

const BACKGROUND_SWATCHES = [
  "#ffffff",
  "#f7f5f0",
  "#eee6d8",
  "#d9dbd4",
  "#2c2924",
  "#1d2a3a",
] as const;

const FRAME_BORDER_SWATCHES = ["#FFFFFF", "#2C2924", "#C5A46D"] as const;

const NEW_PROJECT_SCOPE_PRESENTATION = {
  accessiblePreviewLabel: "Reprodução da Lâmina",
  externalSelection: true,
  scopeControlsLabel: "Escopo da personalização",
  technicalGuides: true,
} as const;

function FrameRangeControl({
  dataPlaceholderFeature,
  label,
  max,
  min,
  onChange,
  step,
  value,
  valueText,
  visibleLabel,
}: {
  dataPlaceholderFeature?: string;
  label: string;
  max: number;
  min: number;
  onChange(value: number): void;
  step: number;
  value: number;
  valueText: string;
  visibleLabel: string;
}) {
  return (
    <div
      className="ui-range-control"
      data-placeholder-feature={dataPlaceholderFeature}
    >
      <div className="ui-range-control__heading">
        <span>{visibleLabel}</span>
        <output>{valueText}</output>
      </div>
      <input
        aria-label={label}
        max={max}
        min={min}
        onChange={(event) => onChange(Number(event.target.value))}
        className="ui-range"
        step={step}
        type="range"
        value={value}
      />
    </div>
  );
}

function personalizationScopeLabel(
  scope: NewProjectPersonalizationDraft["fixedScope"],
) {
  if (scope === "left") return "Página esquerda";
  if (scope === "right") return "Página direita";
  return "Ambos os lados";
}
