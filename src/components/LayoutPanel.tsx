import { useState } from "react";
import { LockKeyhole, LockKeyholeOpen } from "lucide-react";
import type { ComposedSheet, DisplayUnit, FrameOrientation, LayoutSettings } from "../domain/project";
import { createPhysicalFieldDraft, displayUnitLabel, editPhysicalFieldDraft } from "../application/physicalMeasurements";
import { AppIcon, FieldValidationAutoTooltip, FieldValidationTooltip, TextInput,
  fieldValidationTooltipAttributes, useFieldValidationTooltip } from "../ui";
import { SheetPreviewShell } from "./SheetPreview";
import type { LayoutPanelController } from "./useLayoutPanel";
import "./LayoutPanel.css";

interface LayoutPanelProps {
  controller: LayoutPanelController;
  sheet: ComposedSheet;
  mediaPreviewUrls: Readonly<Record<string, string>>;
  presentationUnit: DisplayUnit;
}

export function LayoutPanel({ controller, sheet, mediaPreviewUrls, presentationUnit }: LayoutPanelProps) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { query } = controller;
  const count = query?.frameCount ?? sheet.frames.length;
  const emptyMessage = query?.listing.generationStatus === "empty"
    ? "Escolha a quantidade de posições para preparar um Layout travado."
    : query?.listing.generationStatus === "outsideCoverage"
      ? "As sugestões automáticas atendem de 1 a 30 Frames."
      : "Nenhuma sugestão atende às medidas atuais.";
  return (
    <section aria-label="Painel de Layouts" className="layout-panel" id="layout-panel"
      onKeyDown={(event) => {
        if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); controller.close(); }
      }}>
      <div className="layout-panel__header">
        <strong>Layouts</strong>
        <span>Lâmina {String(sheet.number).padStart(2, "0")} · {count} {count === 1 ? "Frame" : "Frames"}</span>
        <label className="layout-panel__positions">Posições
          <select aria-label="Posições do Layout" value={controller.positionCount}
            disabled={controller.committing || query?.locked || count > 30}
            onChange={(event) => controller.configurePositions(Number(event.target.value), controller.newFrameOrientation)}>
            {Array.from({ length: Math.max(1, 31 - count) }, (_, index) => count + index).map((value) =>
              <option key={value} value={value}>{value}</option>)}
          </select>
        </label>
        {controller.positionCount > count && <label className="layout-panel__positions">Novas posições
          <select aria-label="Orientação das novas posições" value={controller.newFrameOrientation} disabled={controller.committing}
            onChange={(event) => controller.configurePositions(controller.positionCount, event.target.value as FrameOrientation)}>
            <option value="horizontal">Horizontais</option><option value="vertical">Verticais</option><option value="square">Quadradas</option>
          </select>
        </label>}
        <button aria-expanded={settingsOpen} disabled={!query || controller.committing} type="button"
          onClick={() => setSettingsOpen((value) => !value)}>Ajustes</button>
        <button aria-label="Fechar Painel de Layouts" className="layout-panel__close" type="button"
          onClick={controller.close}>×</button>
      </div>
      {settingsOpen && query && <LayoutSettingsForm key={JSON.stringify([query.settings, presentationUnit])}
        settings={query.settings} disabled={controller.committing}
        unit={presentationUnit}
        onApply={controller.updateSettings} />}
      {(["automatic", "custom"] as const).map((origin) => {
        const candidates = query?.listing.candidates.map((candidate, index) => ({ candidate, index }))
          .filter(({ candidate }) => candidate.layout.origin === origin) ?? [];
        return <div className="layout-panel__section" key={origin}>
          <h3>{origin === "automatic" ? "Automáticos" : "Personalizados"}</h3>
          <div className="layout-panel__strip" onPointerLeave={controller.cancelPreview}>
            {candidates.map(({ candidate, index }) => {
              const locked = query!.locked && index === 0;
              const unavailable = query!.locked && !locked;
              const extraPositions = candidate.layout.definition.positions.length - count;
              const caption = locked ? "Layout travado" : extraPositions > 0 ? `${candidate.layout.definition.positions.length} posições · travar`
                : candidate.isLastApplied ? "Último aplicado" : candidate.layout.definition.scope === "page" ? "Por Página" : "Por Lâmina";
              return <div className={`layout-panel__candidate${locked ? " layout-panel__candidate--locked" : ""}${unavailable ? " layout-panel__candidate--unavailable" : ""}`}
                key={`${query!.queryId}-${index}`} onPointerEnter={() => controller.preview(index)}>
              <button
              aria-label={`Aplicar Layout ${index + 1}${candidate.isLastApplied ? " — último aplicado" : ""}`}
              className="layout-panel__preview" disabled={controller.committing || query!.locked || extraPositions > 0} type="button"
              onFocus={() => controller.preview(index)} onBlur={controller.cancelPreview}
              onClick={() => { void controller.apply(index); }}
              title={extraPositions > 0 ? "Use o cadeado para aplicar e criar as posições adicionais." : `${candidate.layout.definition.scope === "page" ? "Por Página" : "Por Lâmina"}${candidate.isLastApplied ? " · Último aplicado" : ""}`}>
              <SheetPreviewShell sheet={{ ...sheet, frames: controller.previews[index] }}
                mediaPreviewUrls={mediaPreviewUrls} />
              </button>
              <button className="layout-panel__lock" type="button" disabled={controller.committing || unavailable}
                aria-label={locked ? `Destravar Layout da Lâmina ${String(sheet.number).padStart(2, "0")}` : `Aplicar e travar Layout ${index + 1}`}
                title={locked ? "Destravar Layout" : "Aplicar e travar Layout"}
                onFocus={() => controller.preview(index)} onBlur={controller.cancelPreview}
                onClick={() => { void (locked ? controller.unlock() : controller.lock(index)); }}>
                <AppIcon icon={locked ? LockKeyhole : LockKeyholeOpen} size={12} />
              </button>
              <span>{caption}</span>
            </div>; })}
            {candidates.length === 0 && <p role="status">{origin === "custom" ? "Nenhum Layout personalizado."
              : controller.error ?? (query ? emptyMessage : "Consultando Layouts…")}</p>}
          </div>
        </div>;
      })}
    </section>
  );
}

function LayoutSettingsForm({ settings, disabled, onApply, unit }: {
  settings: LayoutSettings;
  disabled: boolean;
  unit: DisplayUnit;
  onApply(settings: LayoutSettings): Promise<boolean>;
}) {
  const [permission, setPermission] = useState(settings.permission);
  const [fields, setFields] = useState({
    marginUm: createPhysicalFieldDraft(settings.marginUm, unit),
    gapUm: createPhysicalFieldDraft(settings.gapUm, unit),
    minimumSideUm: createPhysicalFieldDraft(settings.minimumSideUm, unit),
  });
  const entries = ([
    ["marginUm", "Margem"], ["gapUm", "Intervalo"], ["minimumSideUm", "Menor lado"],
  ] as const).map(([field, label]) => ({ field, label, messages:
    !fields[field].hasExactValue || fields[field].valueUm < 0
      ? ["Informe uma medida válida, maior ou igual a zero."]
      : field === "minimumSideUm" && fields[field].valueUm === 0
        ? ["O menor lado deve ser maior que zero."] : [],
  }));
  const tooltip = useFieldValidationTooltip("layout-settings-validation-summary", entries);
  const valid = entries.every(({ messages }) => messages.length === 0);
  return <form className="layout-panel__settings" onSubmit={(event) => {
    event.preventDefault();
    if (valid) void onApply({ permission, marginUm: fields.marginUm.valueUm,
      gapUm: fields.gapUm.valueUm, minimumSideUm: fields.minimumSideUm.valueUm });
  }}>
    <FieldValidationTooltip tooltip={tooltip} />
    <label>Permitir
      <select disabled={disabled} value={permission} onChange={(event) =>
        setPermission(event.target.value as LayoutSettings["permission"])}>
        <option value="pagesOnly">Por Página</option>
        <option value="pagesAndSheet">Por Página e por Lâmina</option>
      </select>
    </label>
    {entries.map(({ field, label, messages }) => <label key={field}>{label} ({displayUnitLabel(unit)})
      <TextInput aria-label={`${label} (${displayUnitLabel(unit)})`} className="ui-field-control"
        {...fieldValidationTooltipAttributes(field, messages[0], tooltip)}
        disabled={disabled} type="text" inputMode="decimal"
        value={fields[field].text} onChange={(event) => setFields({ ...fields,
          [field]: editPhysicalFieldDraft(fields[field], event.target.value, unit) })} />
      <FieldValidationAutoTooltip field={field} tooltip={tooltip} />
    </label>)}
    <button disabled={disabled || !valid} type="submit">Atualizar sugestões</button>
  </form>;
}
