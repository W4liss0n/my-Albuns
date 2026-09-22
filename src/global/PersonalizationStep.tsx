import { FrameDefaultRangeControl } from "../components/FrameDefaultRangeControl";
import { useState } from "react";
import { Image as ImageIcon, X } from "lucide-react";

import {
  changeFrameBorderColor as transitionFrameBorderColor,
  changeFrameBorderWidth as transitionFrameBorderWidth,
} from "../application/frameBorderEditor";
import type {
  ProvisionalDecorativeSelectionOutcome,
} from "./application/globalProjectPort";
import {
  type NewProjectDimensionsDraft,
} from "./application/newProjectDimensions";
import {
  clearOverlay,
  fixPersonalizationScope,
  readBackgroundForFixedScope,
  readOverlayForFixedScope,
  setBackgroundColor,
  setBackgroundImage,
  setOverlayImage,
  type NewProjectPersonalizationDraft,
} from "./application/newProjectPersonalization";
import { personalizationPreviewFromDraft } from "./newProjectPersonalizationPreview";
import { ActionButton, AppIcon } from "../ui";
import { PersonalizationScopeSurface } from "../ui/visualPreview";
import { NewProjectPreviewPanel } from "./NewProjectPreviewPanel";

interface PersonalizationStepProps {
  draft: NewProjectDimensionsDraft;
  onChange(personalization: NewProjectPersonalizationDraft): void;
  onChooseDecorative(): Promise<
    Exclude<ProvisionalDecorativeSelectionOutcome, { status: "failed" }>
  >;
  personalization: NewProjectPersonalizationDraft;
}

export function PersonalizationStep({
  draft,
  onChange,
  onChooseDecorative,
  personalization,
}: PersonalizationStepProps) {
  const [focusedScope, setFocusedScope] = useState<
    NewProjectPersonalizationDraft["fixedScope"] | null
  >(null);
  const frameGapUm = personalization.frameGapUm;
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
    const outcome = await onChooseDecorative();
    if (outcome.status === "selected") {
      onChange(setBackgroundImage(personalization, outcome.selection));
    }
  };
  const chooseOverlay = async () => {
    const outcome = await onChooseDecorative();
    if (outcome.status === "selected") {
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
        surfaceLabel="Prévia do formato da lâmina"
      >
        {(geometry) => (
          <PersonalizationScopeSurface
            focus={{
              kind: "controlled",
              value: focusedScope,
              onChange: setFocusedScope,
            }}
            frameGapUm={frameGapUm}
            geometry={geometry}
            personalization={personalizationPreviewFromDraft(personalization)}
            presentation={NEW_PROJECT_SCOPE_PRESENTATION}
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
          <h2>Fundo</h2>
          <div
            aria-label="Cores de fundo"
            className="new-project-color-swatches"
            role="group"
          >
            {BACKGROUND_SWATCHES.map((color) => (
              <button
                aria-label={`Usar fundo ${color}`}
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
              <span className="ui-visually-hidden">Cor do fundo</span>
              <input
                aria-label="Cor do fundo"
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
            aria-label="Usar imagem… no fundo"
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
          <h2>Sobreposição</h2>
          <ActionButton
            aria-label="Escolher imagem… de sobreposição"
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
                aria-label="Remover sobreposição"
                density="compact"
                onClick={() => onChange(clearOverlay(personalization))}
                variant="quiet"
              >
                <AppIcon icon={X} size={12} />
                Remover
              </ActionButton>
            </>
          ) : (
            <p className="new-project-native-note">Sem sobreposição</p>
          )}
        </section>
        <section className="new-project-value-group">
          <p className="ui-section-eyebrow new-project-group-eyebrow">
            Todas as lâminas
          </p>
          <h2>Quadros</h2>
          <FrameDefaultRangeControl
            kind="border"
            label="Espessura da borda padrão"
            displayUnit={draft.displayUnit}
            onChange={changeFrameBorderWidth}
            valueUm={frameBorderWidthUm}
          />
          <div
            aria-label="Cores da borda"
            className="new-project-color-swatches new-project-color-swatches--frames"
            role="group"
          >
            {FRAME_BORDER_SWATCHES.map((color) => (
              <button
                aria-label={`Usar cor da borda ${color}`}
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
          <FrameDefaultRangeControl
            kind="gap"
            displayUnit={draft.displayUnit}
            onChange={(frameGapUm) => onChange({ ...personalization, frameGapUm })}
            valueUm={frameGapUm}
          />
        </section>
        <p className="new-project-native-note">
          Na próxima etapa, escolha o nome e onde salvar o projeto.
        </p>
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
  accessiblePreviewLabel: "Reprodução da lâmina",
  externalSelection: true,
  scopeControlsLabel: "Aplicar personalização em",
  technicalGuides: true,
} as const;

function personalizationScopeLabel(
  scope: NewProjectPersonalizationDraft["fixedScope"],
) {
  if (scope === "left") return "Página esquerda";
  if (scope === "right") return "Página direita";
  return "Ambos os lados";
}
