import { useState } from "react";
import { Image as ImageIcon, X } from "lucide-react";

import type {
  ProjectLaunchFailure,
  ProvisionalDecorativeSelectionOutcome,
} from "./application/globalProjectPort";
import type {
  NewProjectDimensionsDraft,
} from "./application/newProjectDimensions";
import {
  backgroundForFixedScope,
  clearOverlay,
  fixPersonalizationScope,
  hoverPersonalizationScope,
  overlayForFixedScope,
  setBackgroundColor,
  setBackgroundImage,
  setFrameBorderColor,
  setFrameBorderEnabled,
  setFrameBorderWidth,
  setOverlayImage,
  type NewProjectPersonalizationDraft,
} from "./application/newProjectPersonalization";
import { ActionButton, AppIcon, InlineNotice } from "../ui";
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
  const selectedBackground = backgroundForFixedScope(personalization);
  const backgroundColor =
    selectedBackground.kind === "color" ? selectedBackground.rgb : "#FFFFFF";
  const highlightedScope =
    personalization.hoveredScope ?? personalization.fixedScope;
  const selectedOverlay = overlayForFixedScope(personalization);
  const scopeLabel = personalizationScopeLabel(personalization.fixedScope);
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
        surfaceLabel="Prévia do formato da Lâmina"
      >
        {({ bleedUm, heightUm, safetyUm, widthUm }) => (
          <>
            <PersonalizationPreview
              bleedUm={bleedUm}
              heightUm={heightUm}
              personalization={personalization}
              safetyUm={safetyUm}
              widthUm={widthUm}
            />
            <div
              aria-label="Escopo da personalização"
              className="new-project-scope-controls"
              role="group"
            >
              {(
                [
                  ["left", "Lado esquerdo"],
                  ["both", "Ambos os lados"],
                  ["right", "Lado direito"],
                ] as const
              ).map(([scope, label]) => (
                <button
                  aria-label={label}
                  aria-pressed={personalization.fixedScope === scope}
                  data-highlighted={
                    highlightedScope === scope ? "true" : undefined
                  }
                  key={scope}
                  onClick={() =>
                    onChange(fixPersonalizationScope(personalization, scope))
                  }
                  onPointerEnter={() =>
                    onChange(hoverPersonalizationScope(personalization, scope))
                  }
                  onPointerLeave={() =>
                    onChange(hoverPersonalizationScope(personalization, null))
                  }
                  type="button"
                />
              ))}
            </div>
          </>
        )}
      </NewProjectPreviewPanel>
      <div className="new-project-visual-values">
        <p className="new-project-scope-label">{scopeLabel}</p>
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
            density="compact"
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
            density="compact"
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
          <p className="new-project-group-eyebrow">Todas as Lâminas</p>
          <h2>Frames</h2>
          <label className="new-project-toggle">
            <input
              aria-label="Borda dos Frames"
              checked={personalization.frameBorder.kind === "solid"}
              onChange={(event) =>
                onChange(
                  setFrameBorderEnabled(
                    personalization,
                    event.target.checked,
                  ),
                )
              }
              type="checkbox"
            />
            <span>Borda padrão</span>
          </label>
          {personalization.frameBorder.kind === "solid" ? (
            <div className="new-project-frame-border-fields">
              <label className="new-project-field">
                <span>Cor da Borda</span>
                <input
                  onChange={(event) =>
                    onChange(
                      setFrameBorderColor(
                        personalization,
                        event.target.value,
                      ),
                    )
                  }
                  type="color"
                  value={personalization.frameBorder.rgb}
                />
              </label>
              <label className="new-project-field">
                <span>Espessura da Borda (µm)</span>
                <input
                  inputMode="numeric"
                  min="1"
                  onChange={(event) =>
                    onChange(
                      setFrameBorderWidth(
                        personalization,
                        Number(event.target.value),
                      ),
                    )
                  }
                  step="1"
                  type="number"
                  value={personalization.frameBorder.widthUm}
                />
              </label>
            </div>
          ) : null}
        </section>
        <p className="new-project-native-note">
          Nome e Localização serão escolhidos no diálogo do Windows ao criar.
        </p>
        {pickerFailure ? (
          <InlineNotice
            role="alert"
            title="Não foi possível escolher a Imagem decorativa"
            tone="error"
          >
            <p>{pickerFailure.message}</p>
            {pickerFailure.action ? <p>{pickerFailure.action}</p> : null}
          </InlineNotice>
        ) : null}
        {failure ? (
          <InlineNotice
            role="alert"
            title="Não foi possível criar o Projeto"
            tone="error"
          >
            <p>{failure.message}</p>
            {failure.action ? <p>{failure.action}</p> : null}
          </InlineNotice>
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

function personalizationScopeLabel(
  scope: NewProjectPersonalizationDraft["fixedScope"],
) {
  if (scope === "left") return "Página esquerda";
  if (scope === "right") return "Página direita";
  return "Ambos os lados";
}
