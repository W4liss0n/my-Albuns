import { useEffect, useLayoutEffect, useState } from "react";
import { X } from "lucide-react";

import type {
  AlbumInformation,
  AlbumInformationImpact,
  AlbumInformationValidation,
  DocumentSnapshot,
  DisplayUnit,
  EndSheetFormat,
  SheetSnapshot,
} from "../domain/project";
import {
  createPhysicalFieldDraft,
  displayUnitLabel,
  editPhysicalFieldDraft,
  formatPhysicalMeasurement,
  type PhysicalFieldDraft,
} from "../application/physicalMeasurements";
import {
  invalidPhysicalMeasurementMessage,
  parseIntegerText,
  presentConfigurationValidationErrors,
  type ProjectConfigurationErrors as DimensionsErrors,
} from "../application/projectConfigurationFields";
import {
  createAlbumInformationProjectDraft,
  type AlbumInformationProjectDraft,
} from "../application/projectSettingsDraft";
import {
  AppIcon,
  FieldValidationAutoTooltip,
  FieldValidationTooltip,
  TextInput,
  fieldValidationTooltipAttributes,
  type FieldValidationTooltipModel,
  useFieldValidationTooltip,
} from "../ui";
import { useSemanticBaseline } from "./useSemanticBaseline";
import "./AlbumInformationForm.css";

interface AlbumInformationFormProps {
  document: DocumentSnapshot;
  formId: string;
  revision: number;
  sheetStates: readonly SheetSnapshot[];
  onApply(
    draft: AlbumInformationProjectDraft,
    impact: AlbumInformationImpact,
  ): Promise<boolean>;
  onReadyChange(ready: boolean): void;
  onPresentationUnitChange(unit: DisplayUnit | null): void;
  onValidate(
    information: AlbumInformation,
  ): Promise<AlbumInformationValidation>;
}

interface AlbumInformationDraft {
  displayUnit: DocumentSnapshot["displayUnit"];
  sheetWidth: PhysicalFieldDraft;
  sheetHeight: PhysicalFieldDraft;
  dpi: string;
  bleed: PhysicalFieldDraft;
  safety: PhysicalFieldDraft;
  firstSheet: EndSheetFormat;
  lastSheet: EndSheetFormat;
}

interface AlbumInformationDraftSession {
  current: AlbumInformationDraft;
  pending: {
    attempt: AlbumInformationProjectDraft;
    submitted: AlbumInformationProjectDraft;
    submittedFields: AlbumInformationDraft;
  } | null;
}

type MeasurementDraftKey =
  | "sheetWidth"
  | "sheetHeight"
  | "bleed"
  | "safety";

const UNIT_OPTIONS = [
  { value: "mm", label: "mm" },
  { value: "cm", label: "cm" },
  { value: "in", label: "pol" },
] as const;

const END_SHEET_OPTIONS = [
  { value: "double", label: "Lâmina dupla" },
  { value: "singlePage", label: "Página única" },
] as const;

export function AlbumInformationForm({
  document,
  formId,
  revision,
  sheetStates,
  onApply,
  onPresentationUnitChange,
  onReadyChange,
  onValidate,
}: AlbumInformationFormProps) {
  const projectedBaseline = createDraft(document, sheetStates);
  const semanticBaseline = useSemanticBaseline(
    { fields: projectedBaseline, revision },
    JSON.stringify(toCandidate(projectedBaseline).information),
  );
  const baseline = semanticBaseline.fields;
  const [draftSession, setDraftSession] = useState<AlbumInformationDraftSession>(
    () => ({ current: baseline, pending: null }),
  );
  const draft = draftSession.current;
  const [validated, setValidated] = useState<{
    key: string;
    errors: DimensionsErrors;
    impact: AlbumInformationImpact | null;
  } | null>(null);
  const [validationFailed, setValidationFailed] = useState(false);
  const [applying, setApplying] = useState(false);

  useEffect(() => {
    setDraftSession((session) =>
      rebaseDraftSession(
        session,
        semanticBaseline.revision,
        baseline,
      ),
    );
  }, [baseline, semanticBaseline.revision]);

  const local = toCandidate(draft);
  const candidate = local.information;
  const baselineInformation = toCandidate(baseline).information;
  const candidateKey = candidate ? JSON.stringify(candidate) : "";
  const projectDraft =
    candidate && baselineInformation
      ? createAlbumInformationProjectDraft(
          semanticBaseline.revision,
          baselineInformation,
        ).transition(candidate)
      : null;
  const dirty = projectDraft?.changed ?? false;

  useEffect(() => {
    if (!candidate || !dirty || Object.keys(local.errors).length > 0) {
      setValidated(null);
      setValidationFailed(false);
      return;
    }

    let current = true;
    setValidated(null);
    setValidationFailed(false);
    void onValidate(candidate)
      .then((result) => {
        if (!current) return;
        setValidated({
          key: candidateKey,
          errors: presentConfigurationValidationErrors(result.errors, {
            displayUnit: candidate.displayUnit,
            dpi: candidate.dpi,
            sheetWidthPresentation: "openSheet",
          }),
          impact: result.impact,
        });
      })
      .catch(() => {
        if (current) setValidationFailed(true);
      });
    return () => {
      current = false;
    };
  }, [candidateKey, dirty, onValidate]);

  const validationCurrent = validated?.key === candidateKey;
  const errors = mergeErrors(
    local.errors,
    validationCurrent ? validated.errors : {},
  );
  const validationTooltip = useFieldValidationTooltip(
    `${formId}-validation-summary`,
    [
      { field: "dpi", messages: errors.dpi },
      { field: "sheetWidth", messages: errors.sheetWidth },
      { field: "sheetHeight", messages: errors.sheetHeight },
      { field: "bleed", messages: errors.bleed },
      { field: "safety", messages: errors.safety },
    ],
  );
  const ready = Boolean(
    candidate &&
      dirty &&
      validationCurrent &&
      validated.impact &&
      Object.keys(errors).length === 0 &&
      !validationFailed &&
      !applying,
  );

  useLayoutEffect(() => {
    onReadyChange(ready);
  }, [onReadyChange, ready]);

  useLayoutEffect(() => {
    onPresentationUnitChange(draft.displayUnit);
  }, [draft.displayUnit, onPresentationUnitChange]);

  useLayoutEffect(
    () => () => {
      onPresentationUnitChange(null);
      onReadyChange(false);
    },
    [onPresentationUnitChange, onReadyChange],
  );

  async function submit() {
    if (
      !candidate ||
      !projectDraft ||
      !validated?.impact ||
      !ready
    ) {
      return;
    }
    const submitted = projectDraft;
    setDraftSession((session) => ({
      ...session,
      pending: {
        attempt: submitted,
        submitted,
        submittedFields: session.current,
      },
    }));
    setApplying(true);
    let completed = false;
    try {
      completed = await onApply(submitted, validated.impact);
    } finally {
      if (!completed) {
        setDraftSession((session) =>
          session.pending?.attempt === submitted
            ? { current: session.current, pending: null }
            : session,
        );
      }
      setApplying(false);
    }
  }

  function setField<Key extends keyof AlbumInformationDraft>(
    key: Key,
    value: AlbumInformationDraft[Key],
  ) {
    setDraftSession((session) => ({
      ...session,
      current: { ...session.current, [key]: value },
    }));
  }

  function setMeasurement(
    key: MeasurementDraftKey,
    text: string,
  ) {
    setDraftSession((session) => ({
      ...session,
      current: {
        ...session.current,
        [key]: editPhysicalFieldDraft(
          session.current[key],
          text,
          session.current.displayUnit,
        ),
      },
    }));
  }

  function measurementChanged(key: MeasurementDraftKey) {
    const original = createPhysicalFieldDraft(
      baseline[key].valueUm,
      draft.displayUnit,
    );
    return (
      draft[key].text !== original.text ||
      draft[key].hasExactValue !== original.hasExactValue ||
      draft[key].valueUm !== original.valueUm
    );
  }

  function resetMeasurement(key: MeasurementDraftKey) {
    setDraftSession((session) => ({
      ...session,
      current: {
        ...session.current,
        [key]: createPhysicalFieldDraft(
          baseline[key].valueUm,
          session.current.displayUnit,
        ),
      },
    }));
  }

  function measurementResetAction(key: MeasurementDraftKey) {
    return measurementChanged(key) ? () => resetMeasurement(key) : undefined;
  }

  function changeUnit(unit: DocumentSnapshot["displayUnit"]) {
    setDraftSession((session) => {
      const current = session.current;
      if (unit === current.displayUnit) return session;
      return {
        ...session,
        current: {
          ...current,
          displayUnit: unit,
          sheetWidth: createPhysicalFieldDraft(current.sheetWidth.valueUm, unit),
          sheetHeight: createPhysicalFieldDraft(
            current.sheetHeight.valueUm,
            unit,
          ),
          bleed: createPhysicalFieldDraft(current.bleed.valueUm, unit),
          safety: createPhysicalFieldDraft(current.safety.valueUm, unit),
        },
      };
    });
  }

  const pageWidth = draft.sheetWidth.hasExactValue
    ? draft.sheetWidth.valueUm
    : undefined;
  const pageHeight = draft.sheetHeight.hasExactValue
    ? draft.sheetHeight.valueUm
    : undefined;
  const pageDimensionValid =
    pageWidth !== undefined && pageWidth > 0 && pageWidth % 2 === 0;

  return (
    <form
      id={formId}
      className="album-information-form"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
      }}
    >
      <FieldValidationTooltip tooltip={validationTooltip} />
      <section className="inspector-subsection">
        <h3>Estrutura</h3>
        <div className="inspector-readout-grid">
          <SelectField
            label="Primeira Lâmina"
            value={draft.firstSheet}
            options={END_SHEET_OPTIONS}
            onChange={(value) => setField("firstSheet", value as EndSheetFormat)}
          />
          <SelectField
            label="Última Lâmina"
            value={draft.lastSheet}
            options={END_SHEET_OPTIONS}
            onChange={(value) => setField("lastSheet", value as EndSheetFormat)}
          />
        </div>
      </section>

      <section className="inspector-subsection">
        <h3>Documento</h3>
        <div className="document-compact-controls">
          <SelectField
            label="Unidade"
            value={draft.displayUnit}
            options={UNIT_OPTIONS}
            onChange={(value) =>
              changeUnit(value as DocumentSnapshot["displayUnit"])
            }
          />
          <TextField
            error={firstError(errors.dpi)}
            field="dpi"
            inputMode="numeric"
            label="DPI"
            onReset={
              draft.dpi === baseline.dpi
                ? undefined
                : () => setField("dpi", baseline.dpi)
            }
            validationTooltip={validationTooltip}
            value={draft.dpi}
            onChange={(value) => setField("dpi", value)}
          />
        </div>
        <fieldset className="album-information-dimension">
          <legend>Dimensão da Lâmina</legend>
          <div className="inspector-readout-grid">
            <MeasurementField
              error={firstError(errors.sheetWidth)}
              field="sheetWidth"
              label="Largura"
              onReset={measurementResetAction("sheetWidth")}
              unit={draft.displayUnit}
              validationTooltip={validationTooltip}
              value={draft.sheetWidth.text}
              onChange={(value) => setMeasurement("sheetWidth", value)}
            />
            <MeasurementField
              error={firstError(errors.sheetHeight)}
              field="sheetHeight"
              label="Altura"
              onReset={measurementResetAction("sheetHeight")}
              unit={draft.displayUnit}
              validationTooltip={validationTooltip}
              value={draft.sheetHeight.text}
              onChange={(value) => setMeasurement("sheetHeight", value)}
            />
          </div>
        </fieldset>
        <div aria-label="Dimensão da Página" className="inspector-dimension" role="group">
          <span className="inspector-dimension__title">Dimensão da Página</span>
          <div className="inspector-readout-grid">
            <IntegratedReadout
              label="Largura"
              value={
                pageDimensionValid
                  ? formatPhysicalMeasurement(
                      pageWidth / 2,
                      draft.displayUnit,
                    )
                  : "—"
              }
            />
            <IntegratedReadout
              label="Altura"
              value={
                pageHeight !== undefined && pageHeight > 0
                  ? formatPhysicalMeasurement(pageHeight, draft.displayUnit)
                  : "—"
              }
            />
          </div>
        </div>
      </section>

      <section className="inspector-subsection">
        <h3>Áreas técnicas</h3>
        <div className="inspector-readout-grid">
          <MeasurementField
            error={firstError(errors.bleed)}
            field="bleed"
            label="Sangria"
            onReset={measurementResetAction("bleed")}
            unit={draft.displayUnit}
            validationTooltip={validationTooltip}
            value={draft.bleed.text}
            onChange={(value) => setMeasurement("bleed", value)}
          />
          <MeasurementField
            error={firstError(errors.safety)}
            field="safety"
            label="Área de segurança"
            onReset={measurementResetAction("safety")}
            unit={draft.displayUnit}
            validationTooltip={validationTooltip}
            value={draft.safety.text}
            onChange={(value) => setMeasurement("safety", value)}
          />
        </div>
        {validationFailed ? (
          <p className="album-information-form__status" role="alert">
            Não foi possível validar as alterações.
          </p>
        ) : null}
      </section>
    </form>
  );
}

function createDraft(
  document: DocumentSnapshot,
  sheetStates: readonly SheetSnapshot[],
): AlbumInformationDraft {
  return {
    displayUnit: document.displayUnit,
    sheetWidth: createPhysicalFieldDraft(
      document.sheetWidthUm,
      document.displayUnit,
    ),
    sheetHeight: createPhysicalFieldDraft(
      document.sheetHeightUm,
      document.displayUnit,
    ),
    dpi: String(document.dpi),
    bleed: createPhysicalFieldDraft(document.bleedUm, document.displayUnit),
    safety: createPhysicalFieldDraft(document.safetyUm, document.displayUnit),
    firstSheet: endSheetFormat(sheetStates[0]),
    lastSheet: endSheetFormat(sheetStates[sheetStates.length - 1]),
  };
}

function createDraftFromInformation(
  information: Readonly<AlbumInformation>,
): AlbumInformationDraft {
  return {
    displayUnit: information.displayUnit,
    sheetWidth: createPhysicalFieldDraft(
      information.sheetWidthUm,
      information.displayUnit,
    ),
    sheetHeight: createPhysicalFieldDraft(
      information.sheetHeightUm,
      information.displayUnit,
    ),
    dpi: String(information.dpi),
    bleed: createPhysicalFieldDraft(
      information.bleedUm,
      information.displayUnit,
    ),
    safety: createPhysicalFieldDraft(
      information.safetyUm,
      information.displayUnit,
    ),
    firstSheet: information.firstSheet,
    lastSheet: information.lastSheet,
  };
}

function rebaseDraftSession(
  session: AlbumInformationDraftSession,
  baselineRevision: number,
  baseline: AlbumInformationDraft,
): AlbumInformationDraftSession {
  const pending = session.pending;
  if (!pending) return { current: baseline, pending: null };

  const baselineInformation = toCandidate(baseline).information;
  if (!baselineInformation) return { current: baseline, pending: null };
  const submitted = pending.submitted.rebase(
    baselineRevision,
    baselineInformation,
  );
  const submittedFields = createDraftFromInformation(submitted.value);
  const current = preserveSubsequentDraft(
    submittedFields,
    pending.submittedFields,
    session.current,
  );
  if (!submitted.changed) return { current, pending: null };
  return {
    current,
    pending: {
      attempt: pending.attempt,
      submitted,
      submittedFields,
    },
  };
}

function preserveSubsequentDraft(
  rebasedSubmitted: AlbumInformationDraft,
  submitted: AlbumInformationDraft,
  current: AlbumInformationDraft,
): AlbumInformationDraft {
  const preserved = <Key extends keyof AlbumInformationDraft>(key: Key) =>
    JSON.stringify(current[key]) === JSON.stringify(submitted[key])
      ? rebasedSubmitted[key]
      : current[key];
  return {
    displayUnit: preserved("displayUnit"),
    sheetWidth: preserved("sheetWidth"),
    sheetHeight: preserved("sheetHeight"),
    dpi: preserved("dpi"),
    bleed: preserved("bleed"),
    safety: preserved("safety"),
    firstSheet: preserved("firstSheet"),
    lastSheet: preserved("lastSheet"),
  };
}

function endSheetFormat(sheet: SheetSnapshot | undefined): EndSheetFormat {
  return sheet?.activeSides === "both" ? "double" : "singlePage";
}

function toCandidate(draft: AlbumInformationDraft): {
  information: AlbumInformation | null;
  errors: DimensionsErrors;
} {
  const sheetWidthUm = draft.sheetWidth.hasExactValue
    ? draft.sheetWidth.valueUm
    : null;
  const sheetHeightUm = draft.sheetHeight.hasExactValue
    ? draft.sheetHeight.valueUm
    : null;
  const bleedUm = draft.bleed.hasExactValue ? draft.bleed.valueUm : null;
  const safetyUm = draft.safety.hasExactValue ? draft.safety.valueUm : null;
  const dpi = parseIntegerText(draft.dpi);
  const errors: DimensionsErrors = {};
  const invalidMeasurement = invalidPhysicalMeasurementMessage(
    draft.displayUnit,
  );
  if (sheetWidthUm === null) {
    errors.sheetWidth = [invalidMeasurement];
  }
  if (sheetHeightUm === null) {
    errors.sheetHeight = [invalidMeasurement];
  }
  if (bleedUm === null) {
    errors.bleed = [invalidMeasurement];
  }
  if (safetyUm === null) {
    errors.safety = [invalidMeasurement];
  }
  if (dpi === null) errors.dpi = ["Informe o DPI como um número inteiro suportado."];
  if (
    sheetWidthUm === null ||
    sheetHeightUm === null ||
    bleedUm === null ||
    safetyUm === null ||
    dpi === null
  ) {
    return { information: null, errors };
  }
  return {
    errors,
    information: {
      displayUnit: draft.displayUnit,
      sheetWidthUm,
      sheetHeightUm,
      dpi,
      bleedUm,
      safetyUm,
      firstSheet: draft.firstSheet,
      lastSheet: draft.lastSheet,
    },
  };
}

function mergeErrors(...sources: DimensionsErrors[]): DimensionsErrors {
  return Object.assign({}, ...sources);
}

function firstError(errors: readonly string[] | undefined) {
  return errors?.[0];
}

function SelectField({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: string;
  options: readonly { value: string; label: string }[];
  onChange(value: string): void;
}) {
  return (
    <label className="album-information-field">
      <span>{label}</span>
      <select
        aria-label={label}
        className="ui-field-control"
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function TextField({
  error,
  field,
  inputMode,
  label,
  onReset,
  validationTooltip,
  value,
  onChange,
}: {
  error?: string;
  field: string;
  inputMode: "decimal" | "numeric";
  label: string;
  onReset?: () => void;
  validationTooltip: FieldValidationTooltipModel;
  value: string;
  onChange(value: string): void;
}) {
  const inputId = `album-information-${field}`;
  return (
    <div className="album-information-field">
      <label htmlFor={inputId}>{label}</label>
      <span className="album-entry-control">
        <TextInput
          aria-label={label}
          className="ui-field-control"
          id={inputId}
          inputMode={inputMode}
          type="text"
          value={value}
          {...fieldValidationTooltipAttributes(
            field,
            error,
            validationTooltip,
          )}
          onChange={(event) => onChange(event.currentTarget.value)}
        />
        {onReset ? <FieldResetButton label={label} onReset={onReset} /> : null}
      </span>
      <FieldValidationAutoTooltip
        field={field}
        tooltip={validationTooltip}
      />
    </div>
  );
}

function MeasurementField({
  error,
  field,
  label,
  onReset,
  unit,
  validationTooltip,
  value,
  onChange,
}: {
  error?: string;
  field: string;
  label: string;
  onReset?: () => void;
  unit: DocumentSnapshot["displayUnit"];
  validationTooltip: FieldValidationTooltipModel;
  value: string;
  onChange(value: string): void;
}) {
  const inputId = `album-information-${field}`;
  return (
    <div className="album-information-field">
      <label htmlFor={inputId}>{label}</label>
      <span className="album-entry-control album-measurement-control">
        <TextInput
          aria-label={label}
          className="ui-field-control"
          id={inputId}
          inputMode="decimal"
          type="text"
          value={value}
          {...fieldValidationTooltipAttributes(
            field,
            error,
            validationTooltip,
          )}
          onChange={(event) => onChange(event.currentTarget.value)}
        />
        {onReset ? <FieldResetButton label={label} onReset={onReset} /> : null}
        <span aria-hidden="true">{displayUnitLabel(unit)}</span>
      </span>
      <FieldValidationAutoTooltip
        field={field}
        tooltip={validationTooltip}
      />
    </div>
  );
}

function FieldResetButton({
  label,
  onReset,
}: {
  label: string;
  onReset(): void;
}) {
  return (
    <button
      aria-label={`Restaurar ${label}`}
      className="album-entry-reset"
      type="button"
      onClick={onReset}
      onPointerDown={(event) => event.preventDefault()}
    >
      <AppIcon icon={X} size={12} />
    </button>
  );
}

function IntegratedReadout({ label, value }: { label: string; value: string }) {
  return (
    <div className="inspector-readout-field">
      <span>{label}</span>
      <output aria-label={label} className="inspector-readout inspector-readout--integrated">
        {value}
      </output>
    </div>
  );
}
