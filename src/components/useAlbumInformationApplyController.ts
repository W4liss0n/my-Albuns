import { useEffect, useMemo, useRef, useState } from "react";
import type { ProjectDialogDetail, ProjectDialogPort } from "../application/projectDialogPort";
import { createProjectDecisions } from "../application/projectDecision";
import type { AlbumInformation, AlbumInformationImpact, SheetSnapshot } from "../domain/project";
import { edgeConversionLossDescription } from "../application/edgeConversionReview";
import type { AlbumInformationProjectDraft } from "../application/projectSettingsDraft";
import { createAlbumInformationReview, type AlbumInformationCommitResult, type AlbumInformationReview } from "../application/albumInformationReview";
import { displayUnitLabel, formatPhysicalMeasurement } from "../application/physicalMeasurements";

interface AlbumInformationApplyControllerOptions {
  sheets: readonly SheetSnapshot[];
  projectDialogPort: ProjectDialogPort;
  onApply(draft: AlbumInformationProjectDraft, confirmedReview: AlbumInformationReview): Promise<AlbumInformationCommitResult>;
  onError(message: string): void;
}

export function useAlbumInformationApplyController(input: AlbumInformationApplyControllerOptions) {
  const [active, setActive] = useState(false);
  const latest = useRef(input);
  latest.current = input;
  const context = useMemo(() => ({ active: true, decisions: createProjectDecisions(input.projectDialogPort) }), [input.projectDialogPort]);
  useEffect(() => {
    context.active = true;
    setActive(false);
    return () => { context.active = false; context.decisions.cancel(); };
  }, [context]);

  async function requestApply(draft: AlbumInformationProjectDraft, impact: AlbumInformationImpact) {
    if (!context.active || context.decisions.busy) return false;
    setActive(true);
    try {
      return await context.decisions.run(false, async (decision) => {
        let review = createAlbumInformationReview(draft.baseline, draft.value, impact, latest.current.sheets);
        while (decision.current) {
          const confirmed = await decision.ask({ kind: "albumInformationConfirmation", busy: false, details: detailsFromReview(review) },
            (action) => action === "confirmAlbumInformation" ? true : action === "cancelAlbumInformation" ? false : undefined);
          if (!confirmed || !await decision.present({ kind: "albumInformationConfirmation", busy: true, details: detailsFromReview(review) })) return false;
          const result = await latest.current.onApply(draft, review);
          if (result.kind !== "reviewRequired") return result.kind === "completed";
          review = result.review;
        }
        return false;
      });
    } catch (error: unknown) {
      if (context.active) latest.current.onError(messageFromError(error));
      return false;
    } finally {
      if (context.active) setActive(false);
    }
  }
  return { active, requestApply };
}

function detailsFromReview(review: AlbumInformationReview) {
  return [...albumInformationDetails(
    review.information,
    review.baseline,
    review.impact,
  ), ...review.conversionLosses.map((loss) => ({
    label: `Remoção na Lâmina ${loss.sheetNumber}`,
    value: edgeConversionLossDescription(loss),
  }))];
}

export function albumInformationDetails(
  information: Readonly<AlbumInformation>,
  baseline: Readonly<AlbumInformation>,
  impact: Readonly<AlbumInformationImpact>,
) {
  const measurement = (valueUm: number) =>
    formatPhysicalMeasurement(valueUm, information.displayUnit);
  const formatEnd = (value: AlbumInformation["firstSheet"]) =>
    value === "double" ? "Lâmina dupla" : "Página única";
  const details: ProjectDialogDetail[] = [];
  const addChange = (
    label: string,
    before: string | number,
    after: string | number,
  ) => details.push({ label, value: `${before} → ${after}` });
  const dimensionsChanged =
    information.sheetWidthUm !== baseline.sheetWidthUm ||
    information.sheetHeightUm !== baseline.sheetHeightUm;
  const rasterChanged = dimensionsChanged || information.dpi !== baseline.dpi;

  if (information.firstSheet !== baseline.firstSheet) {
    addChange(
      "Primeira Lâmina",
      formatEnd(baseline.firstSheet),
      formatEnd(information.firstSheet),
    );
  }
  if (information.lastSheet !== baseline.lastSheet) {
    addChange(
      "Última Lâmina",
      formatEnd(baseline.lastSheet),
      formatEnd(information.lastSheet),
    );
  }
  if (information.displayUnit !== baseline.displayUnit) {
    addChange(
      "Unidade",
      displayUnitLabel(baseline.displayUnit),
      displayUnitLabel(information.displayUnit),
    );
  }
  if (information.dpi !== baseline.dpi) {
    addChange("DPI", baseline.dpi, information.dpi);
  }
  if (information.sheetWidthUm !== baseline.sheetWidthUm) {
    addChange(
      "Largura da Lâmina",
      measurement(baseline.sheetWidthUm),
      measurement(information.sheetWidthUm),
    );
  }
  if (information.sheetHeightUm !== baseline.sheetHeightUm) {
    addChange(
      "Altura da Lâmina",
      measurement(baseline.sheetHeightUm),
      measurement(information.sheetHeightUm),
    );
  }
  if (information.bleedUm !== baseline.bleedUm) {
    addChange(
      "Sangria",
      measurement(baseline.bleedUm),
      measurement(information.bleedUm),
    );
  }
  if (information.safetyUm !== baseline.safetyUm) {
    addChange(
      "Área de segurança",
      measurement(baseline.safetyUm),
      measurement(information.safetyUm),
    );
  }
  if (rasterChanged) {
    details.push({
      label: "Resolução resultante",
      value: `Lâmina ${formatPixels(impact.sheetWidthPx)} × ${formatPixels(impact.heightPx)} px · Página ${formatPixels(impact.pageWidthPx)} × ${formatPixels(impact.heightPx)} px`,
    });
  }
  if (dimensionsChanged) {
    details.push({
      label: "Composição",
      value: impact.dimensionalChange?.proportionChanged
        ? "As fotos manterão a proporção. O recorte poderá ser ajustado."
        : "A composição acompanhará o novo tamanho.",
    });
  }
  return details;
}

function formatPixels(value: number) {
  return new Intl.NumberFormat("pt-BR").format(value);
}

function messageFromError(error: unknown) {
  return error instanceof Error
    ? error.message
    : "Não foi possível abrir a confirmação das Informações do Álbum.";
}
