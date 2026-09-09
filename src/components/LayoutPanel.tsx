import { useState } from "react";
import type { ComposedSheet, DisplayUnit, LayoutSettings } from "../domain/project";
import { createPhysicalFieldDraft, displayUnitLabel, editPhysicalFieldDraft } from "../application/physicalMeasurements";
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
    ? "Adicione Frames para ver sugestões de Layout."
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
            {candidates.map(({ candidate, index }) => <button
              aria-label={`Aplicar Layout ${index + 1}${candidate.isLastApplied ? " — último aplicado" : ""}`}
              className="layout-panel__candidate" disabled={controller.committing}
              key={`${query!.queryId}-${index}`} type="button"
              onPointerEnter={() => controller.preview(index)}
              onFocus={() => controller.preview(index)} onBlur={controller.cancelPreview}
              onClick={() => { void controller.apply(index); }}
              title={`${candidate.layout.definition.scope === "page" ? "Por Página" : "Por Lâmina"}${candidate.isLastApplied ? " · Último aplicado" : ""}`}>
              <SheetPreviewShell sheet={{ ...sheet, frames: controller.previews[index] }}
                mediaPreviewUrls={mediaPreviewUrls} />
              <span>{candidate.isLastApplied ? "Último aplicado" : candidate.layout.definition.scope === "page" ? "Por Página" : "Por Lâmina"}</span>
            </button>)}
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
  const [invalid, setInvalid] = useState(false);
  return <form className="layout-panel__settings" onSubmit={(event) => {
    event.preventDefault();
    const valid = Object.values(fields).every((field) => field.hasExactValue && field.valueUm >= 0) && fields.minimumSideUm.valueUm > 0;
    setInvalid(!valid);
    if (valid) void onApply({ permission, marginUm: fields.marginUm.valueUm,
      gapUm: fields.gapUm.valueUm, minimumSideUm: fields.minimumSideUm.valueUm });
  }}>
    <label>Permitir
      <select disabled={disabled} value={permission} onChange={(event) =>
        setPermission(event.target.value as LayoutSettings["permission"])}>
        <option value="pagesOnly">Por Página</option>
        <option value="pagesAndSheet">Por Página e por Lâmina</option>
      </select>
    </label>
    {([
      ["marginUm", "Margem"], ["gapUm", "Intervalo"], ["minimumSideUm", "Menor lado"],
    ] as const).map(([field, label]) => <label key={field}>{label} ({displayUnitLabel(unit)})
      <input disabled={disabled} type="text" inputMode="decimal" required
        value={fields[field].text} onChange={(event) => setFields({ ...fields,
          [field]: editPhysicalFieldDraft(fields[field], event.target.value, unit) })} />
    </label>)}
    <button disabled={disabled} type="submit">Atualizar sugestões</button>
    {invalid && <span role="alert">Use medidas válidas; o menor lado deve ser maior que zero.</span>}
  </form>;
}
