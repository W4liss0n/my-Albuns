import { useState } from "react";
import { Image as ImageIcon, X } from "lucide-react";

import type {
  ProjectLaunchFailure,
  ProvisionalDecorativeSelectionOutcome,
} from "./application/globalProjectPort";
import {
  displayUnitLabel,
  formatMicrometers,
  type NewProjectDimensionsDraft,
} from "./application/newProjectDimensions";
import {
  backgroundForFixedScope,
  clearOverlay,
  fixPersonalizationScope,
  overlayForFixedScope,
  setBackgroundColor,
  setBackgroundImage,
  setFrameBorderColor,
  setFrameBorderEnabled,
  setFrameBorderWidth,
  setOverlayImage,
  type NewProjectPersonalizationDraft,
} from "./application/newProjectPersonalization";
import { ActionButton, AppIcon, FailureNotice } from "../ui";
import { NewProjectPreviewPanel } from "./NewProjectPreviewPanel";
import { PersonalizationPreview } from "./PersonalizationPreview";

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
  const previewedScope =
    personalization.fixedScope === "both" ||
    personalization.fixedScope === hoveredScope
      ? null
      : hoveredScope;
  // PLACEHOLDER UI: o espaço entre Frames ainda não possui contrato de
  // persistência; a medida física controla somente a reprodução desta etapa.
  const [frameGapUm, setFrameGapUm] = useState(6_000);
  const selectedBackground = backgroundForFixedScope(personalization);
  const backgroundColor =
    selectedBackground.kind === "color" ? selectedBackground.rgb : "#FFFFFF";
  const selectedOverlay = overlayForFixedScope(personalization);
  const activeFrameBorder =
    personalization.frameBorder.kind === "solid"
      ? personalization.frameBorder
      : null;
  const frameBorderColor = activeFrameBorder?.rgb ?? "#FFFFFF";
  const frameBorderWidthUm = activeFrameBorder?.widthUm ?? 0;
  const frameBorderValue = activeFrameBorder
    ? `${formatMicrometers(
        activeFrameBorder.widthUm,
        draft.displayUnit,
      )} ${displayUnitLabel(draft.displayUnit)}`
    : "sem borda";
  const scopeLabel = personalizationScopeLabel(personalization.fixedScope);
  const withSolidFrameBorder = () =>
    activeFrameBorder
      ? personalization
      : setFrameBorderColor(
          setFrameBorderEnabled(personalization, true),
          frameBorderColor,
        );
  const changeFrameBorderWidth = (widthUm: number) => {
    if (widthUm <= 0) {
      onChange(setFrameBorderEnabled(personalization, false));
      return;
    }

    onChange(setFrameBorderWidth(withSolidFrameBorder(), widthUm));
  };
  const changeFrameBorderColor = (rgb: string) => {
    onChange(setFrameBorderColor(withSolidFrameBorder(), rgb));
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
          <>
            <PersonalizationPreview
              frameGapUm={frameGapUm}
              geometry={geometry}
              hoveredScope={previewedScope}
              personalization={personalization}
              focusedScope={focusedScope}
            />
            <div
              aria-hidden="true"
              className={`new-project-fixed-selection new-project-fixed-selection--${personalization.fixedScope}`}
            />
            <div
              aria-label="Escopo da personalização"
              className="new-project-scope-controls"
              role="group"
            >
              {(
                [
                  ["left", "Lado esquerdo"],
                  ["right", "Lado direito"],
                ] as const
              ).map(([scope, label]) => (
                <button
                  aria-label={label}
                  aria-pressed={personalization.fixedScope === scope}
                  key={scope}
                  onBlur={() => setFocusedScope(null)}
                  onClick={() =>
                    onChange(fixPersonalizationScope(personalization, scope))
                  }
                  onFocus={(event) =>
                    setFocusedScope(
                      event.currentTarget.matches(":focus-visible")
                        ? scope
                        : null,
                    )
                  }
                  onPointerEnter={() => setHoveredScope(scope)}
                  onPointerLeave={() =>
                    setHoveredScope((currentScope) =>
                      currentScope === scope ? null : currentScope,
                    )
                  }
                  type="button"
                />
              ))}
            </div>
          </>
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
                  selectedBackground.kind === "color" &&
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
          {selectedBackground.kind === "image" ? (
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
          {selectedOverlay ? (
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
      className="new-project-frame-range-control"
      data-placeholder-feature={dataPlaceholderFeature}
    >
      <div className="new-project-frame-range-heading">
        <span>{visibleLabel}</span>
        <output>{valueText}</output>
      </div>
      <input
        aria-label={label}
        max={max}
        min={min}
        onChange={(event) => onChange(Number(event.target.value))}
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
