import { useState } from "react";

import type {
  ProjectLaunchFailure,
  ProvisionalDecorativeSelectionOutcome,
} from "./application/globalProjectPort";
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
import { PersonalizationPreview } from "./PersonalizationPreview";

interface PersonalizationStepProps {
  failure: ProjectLaunchFailure | null;
  heightUm: number;
  onChange(personalization: NewProjectPersonalizationDraft): void;
  onChooseDecorative(): Promise<ProvisionalDecorativeSelectionOutcome>;
  personalization: NewProjectPersonalizationDraft;
  widthUm: number;
}

export function PersonalizationStep({
  failure,
  heightUm,
  onChange,
  onChooseDecorative,
  personalization,
  widthUm,
}: PersonalizationStepProps) {
  const [pickerFailure, setPickerFailure] =
    useState<ProjectLaunchFailure | null>(null);
  const selectedBackground = backgroundForFixedScope(personalization);
  const backgroundColor =
    selectedBackground.kind === "color" ? selectedBackground.rgb : "#FFFFFF";
  const highlightedScope =
    personalization.hoveredScope ?? personalization.fixedScope;
  const selectedOverlay = overlayForFixedScope(personalization);
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
      <div className="new-project-preview">
        <div
          aria-label="Prévia do formato da Lâmina"
          className="new-project-preview-stage"
          style={{ aspectRatio: `${widthUm} / ${heightUm}` }}
        >
          <PersonalizationPreview
            heightUm={heightUm}
            personalization={personalization}
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
        </div>
      </div>
      <div className="new-project-visual-values">
        <section className="new-project-value-group">
          <h2>Background</h2>
          <label className="new-project-field">
            <span>Cor do Background</span>
            <input
              onChange={(event) =>
                onChange(
                  setBackgroundColor(personalization, event.target.value),
                )
              }
              type="color"
              value={backgroundColor}
            />
          </label>
          <button
            aria-label="Escolher imagem de Background"
            onClick={() => void chooseBackground()}
            type="button"
          >
            Escolher imagem…
          </button>
          {selectedBackground.kind === "image" ? (
            <p className="new-project-selection-name">
              {selectedBackground.selection.displayName}
            </p>
          ) : null}
        </section>
        <section className="new-project-value-group">
          <h2>Overlay</h2>
          <button
            aria-label="Escolher imagem de Overlay"
            onClick={() => void chooseOverlay()}
            type="button"
          >
            Escolher imagem…
          </button>
          {selectedOverlay ? (
            <>
              <p className="new-project-selection-name">
                {selectedOverlay.selection.displayName}
              </p>
              <button
                onClick={() => onChange(clearOverlay(personalization))}
                type="button"
              >
                Remover Overlay
              </button>
            </>
          ) : (
            <p className="new-project-native-note">Sem Overlay</p>
          )}
        </section>
        <section className="new-project-value-group">
          <h2>Padrão dos Frames</h2>
          <label className="new-project-toggle">
            <input
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
            <span>Borda dos Frames</span>
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
          <section className="global-open-error" role="alert">
            <h2>Não foi possível escolher a Imagem decorativa</h2>
            <p>{pickerFailure.message}</p>
            {pickerFailure.action ? <p>{pickerFailure.action}</p> : null}
          </section>
        ) : null}
        {failure ? (
          <section className="global-open-error" role="alert">
            <h2>Não foi possível criar o Projeto</h2>
            <p>{failure.message}</p>
            {failure.action ? <p>{failure.action}</p> : null}
          </section>
        ) : null}
      </div>
    </div>
  );
}
