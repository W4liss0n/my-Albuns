import { forwardRef, useId, useRef, useState } from "react";

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
  dimensionsSummary,
  DIMENSIONS_FIELD_ORDER,
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

type CreationStep = "dimensions" | "personalization";

export function NewProjectFlow({
  onCancel,
  onChooseDecorative = noDecorativeSelection,
  onCreate,
  onReleaseDecorative = ignoreReleasedDecorative,
  onValidate,
}: NewProjectFlowProps) {
  const [step, setStep] = useState<CreationStep>("dimensions");
  const [draft, setDraft] = useState(createDefaultDimensionsDraft);
  const [personalization, setPersonalization] = useState(
    createDefaultPersonalizationDraft,
  );
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
    onCancel();
  };

  const updatePersonalization = (
    nextPersonalization: NewProjectPersonalizationDraft,
  ) => {
    const retainedIds = new Set(
      provisionalSelections(nextPersonalization).map(
        (selection) => selection.selectionId,
      ),
    );
    for (const selection of provisionalSelections(personalization)) {
      if (!retainedIds.has(selection.selectionId)) {
        void onReleaseDecorative(selection.selectionId);
      }
    }
    setPersonalization(nextPersonalization);
  };

  const createProject = async () => {
    const configuration = validatedConfiguration;
    if (!configuration) {
      setValidationAttempted(true);
      setStep("dimensions");
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
    setIsCreating(false);
  };

  return (
    <div className="new-project-backdrop">
      <section
        aria-labelledby="new-project-title"
        aria-modal="true"
        className="new-project-flow"
        role="dialog"
      >
        <header className="new-project-header">
          <div>
            <p className="global-eyebrow">Novo Projeto</p>
            <h1 id="new-project-title">
              {step === "dimensions" ? "Dimensões" : "Personalização"}
            </h1>
          </div>
          <ol aria-label="Etapas da criação" className="new-project-steps">
            <li aria-current={step === "dimensions" ? "step" : undefined}>
              Dimensões
            </li>
            <li
              aria-current={
                step === "personalization" ? "step" : undefined
              }
            >
              Personalização
            </li>
          </ol>
        </header>

        {step === "dimensions" ? (
          <DimensionsStep
            attempted={validationAttempted}
            draft={draft}
            errors={validationErrors}
            fieldRefs={fieldRefs}
            onChange={updateDraft}
            validationFailure={validationFailure}
          />
        ) : (
          <PersonalizationStep
            failure={failure}
            heightUm={draft.sheetHeight.valueUm}
            onChange={updatePersonalization}
            onChooseDecorative={onChooseDecorative}
            personalization={personalization}
            widthUm={draft.sheetWidth.valueUm}
          />
        )}

        <footer className="new-project-footer">
          <div>
            {step === "personalization" ? (
              <button
                disabled={isCreating}
                onClick={() => setStep("dimensions")}
                type="button"
              >
                Voltar
              </button>
            ) : null}
          </div>
          <div className="new-project-footer-actions">
            <button disabled={isCreating} onClick={cancelFlow} type="button">
              Cancelar
            </button>
            {step === "dimensions" ? (
              <button
                className="new-project-primary-action"
                disabled={isValidating}
                onClick={goToPersonalization}
                type="button"
              >
                {isValidating ? "Validando…" : "Próximo"}
              </button>
            ) : (
              <button
                className="new-project-primary-action"
                disabled={isCreating}
                onClick={() => void createProject()}
                type="button"
              >
                {isCreating ? "Criando Projeto…" : "Criar"}
              </button>
            )}
          </div>
        </footer>
      </section>
    </div>
  );
}

function noDecorativeSelection() {
  return Promise.resolve({ status: "cancelled" as const });
}

function ignoreReleasedDecorative() {}

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

function DimensionsStep({
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
    onChange(editPhysicalField(draft, field, text), [field]);
  };
  const registerField = (field: DimensionsFieldName) =>
    (element: HTMLInputElement | null) => {
      if (element) {
        fieldRefs.current[field] = element;
      }
    };

  return (
    <div className="new-project-content new-project-dimensions">
      <FormGroup title="Documento">
        <label className="new-project-field">
          <span>Unidade</span>
          <select
            onChange={(event) =>
              onChange(
                changeDisplayUnit(
                  draft,
                  event.target.value as ProjectDisplayUnit,
                ),
                ["sheetWidth", "sheetHeight", "bleed", "safety"],
              )
            }
            value={draft.displayUnit}
          >
            <option value="mm">mm</option>
            <option value="cm">cm</option>
            <option value="in">in</option>
          </select>
        </label>
        <NumericField
          attempted={attempted}
          error={errors.sheetWidth}
          inputMode="decimal"
          label="Largura da Lâmina"
          onChange={(text) => updatePhysical("sheetWidth", text)}
          ref={registerField("sheetWidth")}
          suffix={draft.displayUnit}
          value={draft.sheetWidth.text}
        />
        <NumericField
          attempted={attempted}
          error={errors.sheetHeight}
          inputMode="decimal"
          label="Altura da Lâmina"
          onChange={(text) => updatePhysical("sheetHeight", text)}
          ref={registerField("sheetHeight")}
          suffix={draft.displayUnit}
          value={draft.sheetHeight.text}
        />
        <NumericField
          attempted={attempted}
          error={errors.dpi}
          inputMode="numeric"
          label="DPI"
          onChange={(dpiText) => onChange({ ...draft, dpiText }, ["dpi"])}
          ref={registerField("dpi")}
          value={draft.dpiText}
        />
      </FormGroup>
      <FormGroup title="Estrutura">
        <NumericField
          attempted={attempted}
          error={errors.sheetCount}
          inputMode="numeric"
          label="Quantidade de Lâminas"
          onChange={(sheetCountText) =>
            onChange({ ...draft, sheetCountText }, ["sheetCount"])
          }
          ref={registerField("sheetCount")}
          value={draft.sheetCountText}
        />
        <SelectField
          label="Primeira Lâmina"
          onChange={(firstSheet) => onChange({ ...draft, firstSheet }, [])}
          value={draft.firstSheet}
        />
        <SelectField
          label="Última Lâmina"
          onChange={(lastSheet) => onChange({ ...draft, lastSheet }, [])}
          value={draft.lastSheet}
        />
      </FormGroup>
      <FormGroup title="Áreas técnicas">
        <NumericField
          attempted={attempted}
          error={errors.bleed}
          inputMode="decimal"
          label="Sangria"
          onChange={(text) => updatePhysical("bleed", text)}
          ref={registerField("bleed")}
          suffix={draft.displayUnit}
          value={draft.bleed.text}
        />
        <NumericField
          attempted={attempted}
          error={errors.safety}
          inputMode="decimal"
          label="Área de segurança"
          onChange={(text) => updatePhysical("safety", text)}
          ref={registerField("safety")}
          suffix={draft.displayUnit}
          value={draft.safety.text}
        />
      </FormGroup>
      <p aria-live="polite" className="new-project-summary">
        {dimensionsSummary(draft)}
      </p>
      {validationFailure ? (
        <section className="global-open-error" role="alert">
          <h2>Não foi possível validar as Dimensões</h2>
          <p>{validationFailure.message}</p>
          {validationFailure.action ? <p>{validationFailure.action}</p> : null}
        </section>
      ) : null}
    </div>
  );
}

function FormGroup({
  children,
  title,
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <section className="new-project-value-group new-project-form-group">
      <h2>{title}</h2>
      <div className="new-project-form-fields">{children}</div>
    </section>
  );
}

interface NumericFieldProps {
  attempted: boolean;
  error?: readonly string[];
  inputMode: "decimal" | "numeric";
  label: string;
  onChange(value: string): void;
  suffix?: string;
  value: string;
}

const NumericField = forwardRef<HTMLInputElement, NumericFieldProps>(
  function NumericField(
    { attempted, error, inputMode, label, onChange, suffix, value },
    ref,
  ) {
    const inputId = useId();
    const errorId = `${inputId}-error`;
    const visibleErrors = attempted ? error : undefined;
    return (
      <div className="new-project-field">
        <label htmlFor={inputId}>{label}</label>
        <span className="new-project-input-with-suffix">
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
          {suffix ? <span aria-hidden="true">{suffix}</span> : null}
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
