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

type AlbumInformationPending =
  | {
      kind: "applying";
      attempt: AlbumInformationProjectDraft;
      localFields: ReadonlySet<keyof AlbumInformationDraft>;
      submitted: AlbumInformationProjectDraft;
      submittedFields: AlbumInformationDraft;
      settled: boolean;
    }
  | {
      kind: "followUp";
      baselineFields: AlbumInformationDraft;
      localFields: ReadonlySet<keyof AlbumInformationDraft>;
    };

interface AlbumInformationDraftSession {
  current: AlbumInformationDraft;
  pending: AlbumInformationPending | null;
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
  const applySettled =
    draftSession.pending?.kind === "applying" &&
    draftSession.pending.settled;
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
  }, [applySettled, baseline, semanticBaseline.revision]);

  const local = toCandidate(draft);
  const candidate = local.information;
  const baselineInformation = toCandidate(baseline).information;
  const candidateKey = candidate ? JSON.stringify(candidate) : "";
  const validationKey = `${revision}:${candidateKey}`;
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
          key: validationKey,
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
  }, [dirty, onValidate, validationKey]);

  const validationCurrent = validated?.key === validationKey;
  const errors = mergeErrors(
    local.errors,
    validationCurrent ? validated.errors : {},
  );
  const validationTooltip = useFieldValidationTooltip(
    `${formId}-validation-summary`,
    [
      { field: "firstSheet", messages: errors.firstSheet },
      { field: "lastSheet", messages: errors.lastSheet },
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
        kind: "applying",
        attempt: submitted,
        localFields: new Set(),
        submitted,
        submittedFields: session.current,
        settled: false,
      },
    }));
    setApplying(true);
    let completed = false;
    try {
      completed = await onApply(submitted, validated.impact);
    } finally {
      setDraftSession((session) => {
        const pending = session.pending;
        if (
          pending?.kind !== "applying" ||
          pending.attempt !== submitted
        ) {
          return session;
        }
        return completed
          ? { ...session, pending: { ...pending, settled: true } }
          : { current: session.current, pending: null };
      });
      setApplying(false);
    }
  }

  function setField<Key extends keyof AlbumInformationDraft>(
    key: Key,
    value: AlbumInformationDraft[Key],
  ) {
    setDraftSession((session) => {
      const current = { ...session.current, [key]: value };
      return {
        current,
        pending: trackLocalField(session.pending, key, current),
      };
    });
  }

  function setMeasurement(
    key: MeasurementDraftKey,
    text: string,
  ) {
    setDraftSession((session) => {
      const current = {
        ...session.current,
        [key]: editPhysicalFieldDraft(
          session.current[key],
          text,
          session.current.displayUnit,
        ),
      };
      return {
        current,
        pending: trackLocalField(session.pending, key, current),
      };
    });
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
    setDraftSession((session) => {
      const current = {
        ...session.current,
        [key]: createPhysicalFieldDraft(
          baseline[key].valueUm,
          session.current.displayUnit,
        ),
      };
      return {
        current,
        pending: trackLocalField(session.pending, key, current),
      };
    });
  }

  function measurementResetAction(key: MeasurementDraftKey) {
    return measurementChanged(key) ? () => resetMeasurement(key) : undefined;
  }

  function endSheetResetAction(key: "firstSheet" | "lastSheet") {
    return draft[key] === baseline[key]
      ? undefined
      : () => setField(key, baseline[key]);
  }

  function changeUnit(unit: DocumentSnapshot["displayUnit"]) {
    setDraftSession((session) => {
      const current = session.current;
      if (unit === current.displayUnit) return session;
      const next = {
        ...current,
        displayUnit: unit,
        sheetWidth: createPhysicalFieldDraft(
          current.sheetWidth.valueUm,
          unit,
        ),
        sheetHeight: createPhysicalFieldDraft(
          current.sheetHeight.valueUm,
          unit,
        ),
        bleed: createPhysicalFieldDraft(current.bleed.valueUm, unit),
        safety: createPhysicalFieldDraft(current.safety.valueUm, unit),
      };
      return {
        current: next,
        pending: trackLocalFields(
          session.pending,
          [
            "displayUnit",
            "sheetWidth",
            "sheetHeight",
            "bleed",
            "safety",
          ],
          next,
        ),
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
  // PLACEHOLDER UI: #31 owns proportional transformation of existing content.
  const dimensionChangeRequiresSafeTransformation = sheetStates.some(
    (sheet) => sheet.frames.length > 0,
  );
  // PLACEHOLDER UI: #32 owns reorganization of composed edge content.
  const firstSheetConversionRequiresCompleteFlow =
    (sheetStates[0]?.frames.length ?? 0) > 0;
  const lastSheetConversionRequiresCompleteFlow =
    (sheetStates[sheetStates.length - 1]?.frames.length ?? 0) > 0;

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
            disabled={firstSheetConversionRequiresCompleteFlow}
            error={firstError(errors.firstSheet)}
            field="firstSheet"
            label="Primeira Lâmina"
            onReset={endSheetResetAction("firstSheet")}
            placeholderFeature="convert-composed-edge"
            validationTooltip={validationTooltip}
            value={draft.firstSheet}
            options={END_SHEET_OPTIONS}
            onChange={(value) => setField("firstSheet", value as EndSheetFormat)}
          />
          <SelectField
            disabled={lastSheetConversionRequiresCompleteFlow}
            error={firstError(errors.lastSheet)}
            field="lastSheet"
            label="Última Lâmina"
            onReset={endSheetResetAction("lastSheet")}
            placeholderFeature="convert-composed-edge"
            validationTooltip={validationTooltip}
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
            field="displayUnit"
            label="Unidade"
            validationTooltip={validationTooltip}
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
              disabled={dimensionChangeRequiresSafeTransformation}
              error={firstError(errors.sheetWidth)}
              field="sheetWidth"
              label="Largura"
              onReset={measurementResetAction("sheetWidth")}
              placeholderFeature="safe-sheet-dimension-change"
              unit={draft.displayUnit}
              validationTooltip={validationTooltip}
              value={draft.sheetWidth.text}
              onChange={(value) => setMeasurement("sheetWidth", value)}
            />
            <MeasurementField
              disabled={dimensionChangeRequiresSafeTransformation}
              error={firstError(errors.sheetHeight)}
              field="sheetHeight"
              label="Altura"
              onReset={measurementResetAction("sheetHeight")}
              placeholderFeature="safe-sheet-dimension-change"
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

  if (pending.kind === "followUp") {
    const current = preserveSubsequentDraft(
      baseline,
      session.current,
      pending.localFields,
    );
    return pending.localFields.size === 0
      ? { current: baseline, pending: null }
      : {
          current,
          pending: {
            kind: "followUp",
            baselineFields: baseline,
            localFields: pending.localFields,
          },
        };
  }

  const baselineInformation = toCandidate(baseline).information;
  if (!baselineInformation) return { current: baseline, pending: null };
  const submitted = pending.submitted.rebase(
    baselineRevision,
    baselineInformation,
  );
  const submittedFields = createDraftFromInformation(submitted.value);
  const current = preserveSubsequentDraft(
    pending.settled && !submitted.changed ? baseline : submittedFields,
    session.current,
    pending.localFields,
  );
  if (pending.settled && !submitted.changed) {
    return pending.localFields.size === 0
      ? { current: baseline, pending: null }
      : {
          current,
          pending: {
            kind: "followUp",
            baselineFields: baseline,
            localFields: pending.localFields,
          },
        };
  }
  return {
    current,
    pending: {
      kind: "applying",
      attempt: pending.attempt,
      localFields: pending.localFields,
      submitted,
      submittedFields,
      settled: pending.settled,
    },
  };
}

function preserveSubsequentDraft(
  rebasedSubmitted: AlbumInformationDraft,
  current: AlbumInformationDraft,
  localFields: ReadonlySet<keyof AlbumInformationDraft>,
): AlbumInformationDraft {
  const displayUnit = localFields.has("displayUnit")
    ? current.displayUnit
    : rebasedSubmitted.displayUnit;
  const measurement = (key: MeasurementDraftKey) => {
    if (!localFields.has(key)) {
      return createPhysicalFieldDraft(
        rebasedSubmitted[key].valueUm,
        displayUnit,
      );
    }
    const local = current[key];
    if (current.displayUnit === displayUnit || !local.hasExactValue) {
      // Invalid input has no exact representation to convert; retain its raw
      // text so the user can repair it in the newly projected Unit.
      return local;
    }
    return createPhysicalFieldDraft(local.valueUm, displayUnit);
  };
  return {
    displayUnit,
    sheetWidth: measurement("sheetWidth"),
    sheetHeight: measurement("sheetHeight"),
    dpi: localFields.has("dpi") ? current.dpi : rebasedSubmitted.dpi,
    bleed: measurement("bleed"),
    safety: measurement("safety"),
    firstSheet: localFields.has("firstSheet")
      ? current.firstSheet
      : rebasedSubmitted.firstSheet,
    lastSheet: localFields.has("lastSheet")
      ? current.lastSheet
      : rebasedSubmitted.lastSheet,
  };
}

function trackLocalField(
  pending: AlbumInformationPending | null,
  key: keyof AlbumInformationDraft,
  current: AlbumInformationDraft,
): AlbumInformationPending | null {
  if (!pending) return null;
  const baseline =
    pending.kind === "applying"
      ? pending.submittedFields
      : pending.baselineFields;
  const localFields = new Set(pending.localFields);
  if (albumInformationFieldEquals(key, current, baseline)) {
    localFields.delete(key);
  } else {
    localFields.add(key);
  }
  return { ...pending, localFields };
}

function trackLocalFields(
  pending: AlbumInformationPending | null,
  keys: readonly (keyof AlbumInformationDraft)[],
  current: AlbumInformationDraft,
) {
  return keys.reduce<AlbumInformationPending | null>(
    (tracked, key) => trackLocalField(tracked, key, current),
    pending,
  );
}

function albumInformationFieldEquals(
  key: keyof AlbumInformationDraft,
  left: AlbumInformationDraft,
  right: AlbumInformationDraft,
) {
  switch (key) {
    case "sheetWidth":
    case "sheetHeight":
    case "bleed":
    case "safety":
      return physicalFieldDraftEquals(left[key], right[key]);
    case "dpi":
      return integerDraftEquals(left.dpi, right.dpi);
    default:
      return left[key] === right[key];
  }
}

function physicalFieldDraftEquals(
  left: PhysicalFieldDraft,
  right: PhysicalFieldDraft,
) {
  if (left.hasExactValue && right.hasExactValue) {
    return left.valueUm === right.valueUm;
  }
  return (
    left.text === right.text &&
    left.hasExactValue === right.hasExactValue &&
    left.valueUm === right.valueUm
  );
}

function integerDraftEquals(left: string, right: string) {
  const leftValue = parseIntegerText(left);
  const rightValue = parseIntegerText(right);
  return leftValue === null || rightValue === null
    ? left === right
    : leftValue === rightValue;
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
  disabled = false,
  error,
  field,
  label,
  onReset,
  value,
  options,
  placeholderFeature,
  validationTooltip,
  onChange,
}: {
  disabled?: boolean;
  error?: string;
  field: string;
  label: string;
  onReset?: () => void;
  value: string;
  options: readonly { value: string; label: string }[];
  placeholderFeature?: string;
  validationTooltip: FieldValidationTooltipModel;
  onChange(value: string): void;
}) {
  const inputId = `album-information-${field}`;
  return (
    <div
      className="album-information-field"
      data-placeholder-feature={disabled ? placeholderFeature : undefined}
      title={
        disabled
          ? "Disponível após a conversão completa de extremidades"
          : undefined
      }
    >
      <label htmlFor={inputId}>{label}</label>
      <span className="album-entry-control">
        <select
          aria-label={label}
          className="ui-field-control"
          disabled={disabled}
          id={inputId}
          value={value}
          {...fieldValidationTooltipAttributes(
            field,
            error,
            validationTooltip,
          )}
          onChange={(event) => onChange(event.currentTarget.value)}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
        {onReset ? <FieldResetButton label={label} onReset={onReset} /> : null}
      </span>
      <FieldValidationAutoTooltip field={field} tooltip={validationTooltip} />
    </div>
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
  disabled = false,
  error,
  field,
  label,
  onReset,
  placeholderFeature,
  unit,
  validationTooltip,
  value,
  onChange,
}: {
  disabled?: boolean;
  error?: string;
  field: string;
  label: string;
  onReset?: () => void;
  placeholderFeature?: string;
  unit: DocumentSnapshot["displayUnit"];
  validationTooltip: FieldValidationTooltipModel;
  value: string;
  onChange(value: string): void;
}) {
  const inputId = `album-information-${field}`;
  return (
    <div
      className="album-information-field"
      data-placeholder-feature={disabled ? placeholderFeature : undefined}
      title={
        disabled
          ? "Disponível após a mudança dimensional segura"
          : undefined
      }
    >
      <label htmlFor={inputId}>{label}</label>
      <span className="album-entry-control album-measurement-control">
        <TextInput
          aria-label={label}
          className="ui-field-control"
          disabled={disabled}
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
