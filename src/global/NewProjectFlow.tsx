import { forwardRef, useId, useRef, useState } from "react";
import { Minus, Plus, Save } from "lucide-react";

import type {
  NewProjectConfiguration,
  ProjectConfigurationValidationOutcome,
  ProjectDisplayUnit,
  ProjectEndSheetFormat,
  ProjectLaunchFailure,
  ProjectLaunchOutcome,
  ProvisionalDecorativeSelectionOutcome,
} from "./application/globalProjectPort";
import {
  changeDisplayUnit,
  createDefaultDimensionsDraft,
  DIMENSIONS_FIELD_ORDER,
  displayUnitLabel,
  editPhysicalField,
  getLocalInputErrors,
  presentConfigurationValidationErrors,
  toNewProjectConfiguration,
  type DimensionsErrors,
  type DimensionsFieldName,
  type NewProjectDimensionsDraft,
  type PhysicalFieldName,
} from "./application/newProjectDimensions";
import {
  createDefaultPersonalizationDraft,
  provisionalSelections,
  toCreationConfiguration,
  type NewProjectCreationConfiguration,
  type NewProjectPersonalizationDraft,
} from "./application/newProjectPersonalization";
import {
  createBuiltInProjectPresets,
  type NewProjectPreset,
} from "./application/newProjectPresets";
import { ActionButton, AppIcon, InlineNotice } from "../ui";
import { DimensionsPreview } from "./DimensionsPreview";
import { PersonalizationStep } from "./PersonalizationStep";
import "./NewProjectFlow.css";

interface NewProjectFlowProps {
  onCancel(): void;
  onChooseDecorative?(): Promise<ProvisionalDecorativeSelectionOutcome>;
  onCreate(
    configuration: NewProjectCreationConfiguration,
  ): Promise<ProjectLaunchOutcome>;
  onValidate(
    configuration: NewProjectConfiguration,
  ): Promise<ProjectConfigurationValidationOutcome>;
  onReleaseDecorative?(selectionId: string): Promise<void> | void;
}

type CreationStep = "configuration" | "personalization";

export function NewProjectFlow({
  onCancel,
  onChooseDecorative = noDecorativeSelection,
  onCreate,
  onReleaseDecorative = ignoreReleasedDecorative,
  onValidate,
}: NewProjectFlowProps) {
  const [step, setStep] = useState<CreationStep>("configuration");
  const [draft, setDraft] = useState(createDefaultDimensionsDraft);
  const [personalization, setPersonalization] = useState(
    createDefaultPersonalizationDraft,
  );
  // PLACEHOLDER UI: coleção local e não persistida; ver newProjectPresets.ts.
  const [presets, setPresets] = useState(createBuiltInProjectPresets);
  const [selectedPresetId, setSelectedPresetId] = useState("");
  const [validationAttempted, setValidationAttempted] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [failure, setFailure] = useState<ProjectLaunchFailure | null>(null);
  const [validationErrors, setValidationErrors] =
    useState<DimensionsErrors>({});
  const [validationFailure, setValidationFailure] =
    useState<ProjectLaunchFailure | null>(null);
  const [isValidating, setIsValidating] = useState(false);
  const [validatedConfiguration, setValidatedConfiguration] =
    useState<NewProjectConfiguration | null>(null);
  const validationRequest = useRef(0);
  const nextCustomPreset = useRef(1);
  const fieldRefs = useRef<
    Partial<Record<DimensionsFieldName, HTMLInputElement>>
  >({});

  const focusFirstInvalid = (errors: DimensionsErrors) => {
    const firstInvalid = DIMENSIONS_FIELD_ORDER.find(
      (field) => errors[field]?.length,
    );
    if (firstInvalid) {
      fieldRefs.current[firstInvalid]?.focus();
    }
  };

  const validateDraft = async (
    candidateDraft: NewProjectDimensionsDraft,
    advanceWhenValid: boolean,
    focusInvalid: boolean,
    changedFields?: readonly DimensionsFieldName[],
  ) => {
    const request = validationRequest.current + 1;
    validationRequest.current = request;
    setValidatedConfiguration(null);
    setValidationFailure(null);

    const localErrors = getLocalInputErrors(candidateDraft);
    setValidationErrors((currentErrors) =>
      changedFields
        ? mergeLiveValidationErrors(
            currentErrors,
            localErrors,
            changedFields,
          )
        : localErrors,
    );
    if (Object.keys(localErrors).length > 0) {
      setIsValidating(false);
      if (focusInvalid) {
        focusFirstInvalid(localErrors);
      }
      return;
    }

    const configuration = toNewProjectConfiguration(candidateDraft);
    if (!configuration) {
      setIsValidating(false);
      return;
    }
    setIsValidating(true);
    const outcome = await onValidate(configuration);
    if (validationRequest.current !== request) {
      return;
    }
    setIsValidating(false);
    if (outcome.status === "failed") {
      setValidationFailure(outcome.error);
      return;
    }
    if (outcome.status === "invalid") {
      const errors = presentConfigurationValidationErrors(outcome.errors);
      setValidationErrors(errors);
      if (focusInvalid) {
        focusFirstInvalid(errors);
      }
      return;
    }

    setValidationErrors({});
    setValidatedConfiguration(configuration);
    if (advanceWhenValid) {
      setStep("personalization");
    }
  };

  const updateDraft = (
    candidateDraft: NewProjectDimensionsDraft,
    changedFields: readonly DimensionsFieldName[],
  ) => {
    setDraft(candidateDraft);
    setSelectedPresetId("");
    setValidatedConfiguration(null);
    if (validationAttempted) {
      void validateDraft(candidateDraft, false, false, changedFields);
    }
  };

  const goToPersonalization = () => {
    setValidationAttempted(true);
    void validateDraft(draft, true, true);
  };

  const cancelFlow = () => {
    validationRequest.current += 1;
    for (const selection of ownedProvisionalSelections(
      personalization,
      presets,
    )) {
      void onReleaseDecorative(selection.selectionId);
    }
    onCancel();
  };

  const updatePersonalization = (
    nextPersonalization: NewProjectPersonalizationDraft,
  ) => {
    const retainedIds = new Set(
      ownedProvisionalSelections(nextPersonalization, presets).map(
        (selection) => selection.selectionId,
      ),
    );
    for (const selection of provisionalSelections(personalization)) {
      if (!retainedIds.has(selection.selectionId)) {
        void onReleaseDecorative(selection.selectionId);
      }
    }
    setPersonalization(nextPersonalization);
    setSelectedPresetId("");
  };

  const applyProjectPreset = (presetId: string) => {
    if (!presetId) {
      setSelectedPresetId("");
      return;
    }
    const preset = presets.find((candidate) => candidate.id === presetId);
    if (!preset) {
      return;
    }
    validationRequest.current += 1;
    setIsValidating(false);
    setValidationFailure(null);
    setValidatedConfiguration(null);
    setDraft(preset.dimensions);
    updatePersonalization(preset.personalization);
    setSelectedPresetId(preset.id);
    if (validationAttempted) {
      void validateDraft(preset.dimensions, false, false);
    } else {
      setValidationErrors({});
    }
  };

  const saveProjectPreset = (name: string) => {
    const preset: NewProjectPreset = {
      id: `custom-${nextCustomPreset.current}`,
      name,
      dimensions: draft,
      personalization,
    };
    nextCustomPreset.current += 1;
    setPresets((current) => [preset, ...current]);
    setSelectedPresetId(preset.id);
  };

  const createProject = async () => {
    const configuration = validatedConfiguration;
    if (!configuration) {
      setValidationAttempted(true);
      setStep("configuration");
      void validateDraft(draft, false, true);
      return;
    }
    setIsCreating(true);
    setFailure(null);
    const outcome = await onCreate(
      toCreationConfiguration(configuration, personalization),
    );
    if (outcome.status === "failed") {
      setFailure(outcome.error);
    }
    if (outcome.status === "opened") {
      const consumedIds = new Set(
        provisionalSelections(personalization).map(
          (selection) => selection.selectionId,
        ),
      );
      for (const selection of provisionalSelectionsInPresets(presets)) {
        if (!consumedIds.has(selection.selectionId)) {
          void onReleaseDecorative(selection.selectionId);
        }
      }
    }
    setIsCreating(false);
  };

  return (
    <main
      aria-labelledby="new-project-step-title"
      className="new-project-flow"
    >
      <header className="new-project-header">
        <h2 className="ui-visually-hidden" id="new-project-step-title">
          {step === "configuration" ? "Configurações" : "Personalização"}
        </h2>
        <ol aria-label="Etapas da criação" className="new-project-steps">
          <li aria-current={step === "configuration" ? "step" : undefined}>
            <span>1</span>
            Configurações
          </li>
          <li
            aria-current={step === "personalization" ? "step" : undefined}
          >
            <span>2</span>
            Personalização
          </li>
        </ol>
        <PresetControl
          onApply={applyProjectPreset}
          onSave={saveProjectPreset}
          presets={presets}
          selectedPresetId={selectedPresetId}
        />
      </header>

      {step === "configuration" ? (
        <ConfigurationStep
          attempted={validationAttempted}
          draft={draft}
          errors={validationErrors}
          fieldRefs={fieldRefs}
          onChange={updateDraft}
          validationFailure={validationFailure}
        />
      ) : (
        <PersonalizationStep
          draft={draft}
          failure={failure}
          onChange={updatePersonalization}
          onChooseDecorative={onChooseDecorative}
          personalization={personalization}
        />
      )}

      <footer className="new-project-footer">
        <div>
          <ActionButton
            disabled={isCreating}
            onClick={cancelFlow}
            variant="quiet"
          >
            Cancelar
          </ActionButton>
        </div>
        <div className="new-project-footer-actions">
          {step === "personalization" ? (
            <ActionButton
              disabled={isCreating}
              onClick={() => setStep("configuration")}
            >
              Voltar
            </ActionButton>
          ) : null}
          {step === "configuration" ? (
            <ActionButton
              aria-label="Continuar"
              disabled={isValidating}
              onClick={goToPersonalization}
              variant="primary"
            >
              {isValidating ? "Validando…" : "Continuar"}
            </ActionButton>
          ) : (
            <ActionButton
              aria-label="Criar"
              disabled={isCreating}
              onClick={() => void createProject()}
              variant="primary"
            >
              {isCreating ? "Criando Projeto…" : "Criar Projeto"}
            </ActionButton>
          )}
        </div>
      </footer>
    </main>
  );
}

function noDecorativeSelection() {
  return Promise.resolve({ status: "cancelled" as const });
}

function ignoreReleasedDecorative() {}

function provisionalSelectionsInPresets(
  presets: readonly NewProjectPreset[],
) {
  const selections = presets.flatMap((preset) =>
    provisionalSelections(preset.personalization),
  );
  return uniqueSelections(selections);
}

function ownedProvisionalSelections(
  personalization: NewProjectPersonalizationDraft,
  presets: readonly NewProjectPreset[],
) {
  return uniqueSelections([
    ...provisionalSelections(personalization),
    ...provisionalSelectionsInPresets(presets),
  ]);
}

function uniqueSelections(
  selections: readonly ReturnType<typeof provisionalSelections>[number][],
) {
  const byId = new Map(
    selections.map((selection) => [selection.selectionId, selection]),
  );
  return [...byId.values()];
}

function mergeLiveValidationErrors(
  currentErrors: DimensionsErrors,
  localErrors: DimensionsErrors,
  changedFields: readonly DimensionsFieldName[],
): DimensionsErrors {
  const nextErrors = { ...currentErrors };
  for (const field of changedFields) {
    delete nextErrors[field];
  }
  for (const field of DIMENSIONS_FIELD_ORDER) {
    const messages = localErrors[field];
    if (messages?.length) {
      nextErrors[field] = messages;
    }
  }
  return nextErrors;
}

function ConfigurationStep({
  attempted,
  draft,
  errors,
  fieldRefs,
  onChange,
  validationFailure,
}: {
  attempted: boolean;
  draft: NewProjectDimensionsDraft;
  errors: DimensionsErrors;
  fieldRefs: React.RefObject<
    Partial<Record<DimensionsFieldName, HTMLInputElement>>
  >;
  onChange(
    draft: NewProjectDimensionsDraft,
    changedFields: readonly DimensionsFieldName[],
  ): void;
  validationFailure: ProjectLaunchFailure | null;
}) {
  const updatePhysical = (field: PhysicalFieldName, text: string) => {
    const validationField =
      field === "closedSheetWidth" ? "sheetWidth" : field;
    onChange(editPhysicalField(draft, field, text), [validationField]);
  };
  const registerField = (field: DimensionsFieldName) =>
    (element: HTMLInputElement | null) => {
      if (element) {
        fieldRefs.current[field] = element;
      }
    };
  const parsedSheetCount = Number.parseInt(draft.sheetCountText, 10);
  const adjustSheetCount = (offset: -2 | 2) => {
    if (!Number.isSafeInteger(parsedSheetCount)) {
      return;
    }
    const sheetCountText = String(parsedSheetCount + offset);
    onChange({ ...draft, sheetCountText }, ["sheetCount"]);
  };

  return (
    <div className="new-project-content new-project-dimensions">
      <DimensionsPreview draft={draft} />
      <div className="new-project-dimensions-controls">
        <ControlSection title="Unidade">
          <UnitSelector
            onChange={(displayUnit) =>
              onChange(changeDisplayUnit(draft, displayUnit), [
                "sheetWidth",
                "sheetHeight",
                "bleed",
                "safety",
              ])
            }
            value={draft.displayUnit}
          />
        </ControlSection>

        <ControlSection title="Dimensão da Lâmina fechada">
          <div className="new-project-size-fields">
            <NumericField
              attempted={attempted}
              error={errors.sheetWidth}
              hideLabel
              inputMode="decimal"
              label="Largura da Lâmina fechada"
              onChange={(text) => updatePhysical("closedSheetWidth", text)}
              ref={registerField("sheetWidth")}
              suffix={displayUnitLabel(draft.displayUnit)}
              value={draft.closedSheetWidth.text}
            />
            <span aria-hidden="true" className="new-project-size-separator">
              ×
            </span>
            <NumericField
              attempted={attempted}
              error={errors.sheetHeight}
              hideLabel
              inputMode="decimal"
              label="Altura da Lâmina fechada"
              onChange={(text) => updatePhysical("sheetHeight", text)}
              ref={registerField("sheetHeight")}
              suffix={displayUnitLabel(draft.displayUnit)}
              value={draft.sheetHeight.text}
            />
          </div>
        </ControlSection>

        <ControlSection title="Sangria e Área de segurança">
          <div className="new-project-paired-fields">
            <NumericField
              attempted={attempted}
              error={errors.bleed}
              inputMode="decimal"
              label="Sangria"
              onChange={(text) => updatePhysical("bleed", text)}
              ref={registerField("bleed")}
              suffix={displayUnitLabel(draft.displayUnit)}
              value={draft.bleed.text}
            />
            <NumericField
              attempted={attempted}
              error={errors.safety}
              inputMode="decimal"
              label="Área de segurança"
              onChange={(text) => updatePhysical("safety", text)}
              ref={registerField("safety")}
              suffix={displayUnitLabel(draft.displayUnit)}
              value={draft.safety.text}
            />
          </div>
        </ControlSection>

        <ControlSection className="new-project-sheet-count" title="Lâminas">
          <NumericField
            attempted={attempted}
            controls={
              <span className="new-project-stepper-actions">
                <button
                  aria-label="Diminuir quantidade de Lâminas"
                  onClick={() => adjustSheetCount(-2)}
                  type="button"
                >
                  <AppIcon icon={Minus} size={12} />
                </button>
                <button
                  aria-label="Aumentar quantidade de Lâminas"
                  onClick={() => adjustSheetCount(2)}
                  type="button"
                >
                  <AppIcon icon={Plus} size={12} />
                </button>
              </span>
            }
            error={errors.sheetCount}
            hideLabel
            inputMode="numeric"
            label="Quantidade de Lâminas"
            onChange={(sheetCountText) =>
              onChange({ ...draft, sheetCountText }, ["sheetCount"])
            }
            ref={registerField("sheetCount")}
            value={draft.sheetCountText}
          />
        </ControlSection>

        <ControlSection title="Resolução do Projeto">
          <NumericField
            attempted={attempted}
            error={errors.dpi}
            hideLabel
            inputMode="numeric"
            label="DPI"
            onChange={(dpiText) =>
              onChange({ ...draft, dpiText }, ["dpi"])
            }
            ref={registerField("dpi")}
            suffix="DPI"
            value={draft.dpiText}
          />
        </ControlSection>

        <ControlSection title="Configuração das extremidades">
          <div className="new-project-paired-fields">
            <SelectField
              label="Primeira Lâmina"
              onChange={(firstSheet) =>
                onChange({ ...draft, firstSheet }, [])
              }
              value={draft.firstSheet}
            />
            <SelectField
              label="Última Lâmina"
              onChange={(lastSheet) => onChange({ ...draft, lastSheet }, [])}
              value={draft.lastSheet}
            />
          </div>
        </ControlSection>

        {validationFailure ? (
          <InlineNotice
            role="alert"
            title="Não foi possível validar as Configurações"
            tone="error"
          >
            <p>{validationFailure.message}</p>
            {validationFailure.action ? (
              <p>{validationFailure.action}</p>
            ) : null}
          </InlineNotice>
        ) : null}
      </div>
    </div>
  );
}

function ControlSection({
  children,
  className,
  dataPlaceholderFeature,
  title,
}: {
  children: React.ReactNode;
  className?: string;
  dataPlaceholderFeature?: string;
  title: string;
}) {
  return (
    <section
      className={`new-project-control-section${className ? ` ${className}` : ""}`}
      data-placeholder-feature={dataPlaceholderFeature}
    >
      <h2>{title}</h2>
      {children}
    </section>
  );
}

function PresetControl({
  onApply,
  onSave,
  presets,
  selectedPresetId,
}: {
  onApply(presetId: string): void;
  onSave(name: string): void;
  presets: readonly NewProjectPreset[];
  selectedPresetId: string;
}) {
  const [isSaving, setIsSaving] = useState(false);
  const [name, setName] = useState("");
  const presetId = useId();
  const presetNameId = useId();

  const confirmSave = () => {
    const trimmedName = name.trim();
    if (!trimmedName) {
      return;
    }
    onSave(trimmedName);
    setName("");
    setIsSaving(false);
  };

  return (
    <ControlSection
      className="new-project-preset-control"
      dataPlaceholderFeature="new-project-presets"
      title="Predefinição"
    >
      <div className="new-project-preset-row">
        <label className="ui-visually-hidden" htmlFor={presetId}>
          Predefinição
        </label>
        <select
          id={presetId}
          onChange={(event) => onApply(event.target.value)}
          value={selectedPresetId}
        >
          <option value="">Nenhuma</option>
          {presets.map((preset) => (
            <option key={preset.id} value={preset.id}>
              {preset.name}
            </option>
          ))}
        </select>
        <button
          aria-label="Salvar configuração atual como predefinição"
          onClick={() => setIsSaving((current) => !current)}
          type="button"
        >
          <AppIcon icon={Save} size={14} />
        </button>
        {isSaving ? (
          <form
            aria-label="Salvar predefinição"
            className="new-project-save-preset"
            onSubmit={(event) => {
              event.preventDefault();
              confirmSave();
            }}
          >
            <strong>Salvar predefinição</strong>
            <label className="ui-visually-hidden" htmlFor={presetNameId}>
              Nome da predefinição
            </label>
            <input
              autoFocus
              id={presetNameId}
              onChange={(event) => setName(event.target.value)}
              placeholder="Nome da predefinição"
              type="text"
              value={name}
            />
            <div>
              <button
                onClick={() => {
                  setName("");
                  setIsSaving(false);
                }}
                type="button"
              >
                Cancelar
              </button>
              <button disabled={!name.trim()} type="submit">
                Salvar
              </button>
            </div>
          </form>
        ) : null}
      </div>
    </ControlSection>
  );
}

const UNIT_OPTIONS: readonly ProjectDisplayUnit[] = ["mm", "cm", "in"];

function UnitSelector({
  onChange,
  value,
}: {
  onChange(value: ProjectDisplayUnit): void;
  value: ProjectDisplayUnit;
}) {
  return (
    <div aria-label="Unidade" className="new-project-unit-selector" role="group">
      {UNIT_OPTIONS.map((option) => (
        <button
          aria-pressed={value === option}
          key={option}
          onClick={() => onChange(option)}
          type="button"
        >
          {displayUnitLabel(option)}
        </button>
      ))}
    </div>
  );
}

interface NumericFieldProps {
  attempted: boolean;
  controls?: React.ReactNode;
  error?: readonly string[];
  hideLabel?: boolean;
  inputMode: "decimal" | "numeric";
  label: string;
  onChange(value: string): void;
  suffix?: string;
  value: string;
}

const NumericField = forwardRef<HTMLInputElement, NumericFieldProps>(
  function NumericField(
    {
      attempted,
      controls,
      error,
      hideLabel,
      inputMode,
      label,
      onChange,
      suffix,
      value,
    },
    ref,
  ) {
    const inputId = useId();
    const errorId = `${inputId}-error`;
    const visibleErrors = attempted ? error : undefined;
    const hasControls = Boolean(controls);
    return (
      <div className="new-project-field">
        <label
          className={hideLabel ? "ui-visually-hidden" : undefined}
          htmlFor={inputId}
        >
          {label}
        </label>
        <span
          className={`new-project-input-shell${suffix ? " new-project-input-shell--suffix" : ""}${hasControls ? " new-project-input-shell--controlled" : ""}`}
        >
          <input
            aria-describedby={visibleErrors?.length ? errorId : undefined}
            aria-invalid={visibleErrors?.length ? true : undefined}
            inputMode={inputMode}
            id={inputId}
            onChange={(event) => onChange(event.target.value)}
            ref={ref}
            type="text"
            value={value}
          />
          {suffix ? (
            <span aria-hidden="true" className="new-project-input-suffix">
              {suffix}
            </span>
          ) : null}
          {controls}
        </span>
        {visibleErrors?.length ? (
          <span className="new-project-field-errors" id={errorId}>
            {visibleErrors.map((message, index) => (
              <small
                className="new-project-field-error"
                key={`${message}-${index}`}
              >
                {message}
              </small>
            ))}
          </span>
        ) : null}
      </div>
    );
  },
);

function SelectField({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange(value: ProjectEndSheetFormat): void;
  value: ProjectEndSheetFormat;
}) {
  return (
    <label className="new-project-field">
      <span>{label}</span>
      <select
        onChange={(event) =>
          onChange(event.target.value as ProjectEndSheetFormat)
        }
        value={value}
      >
        <option value="double">Dupla</option>
        <option value="singlePage">Página única</option>
      </select>
    </label>
  );
}
