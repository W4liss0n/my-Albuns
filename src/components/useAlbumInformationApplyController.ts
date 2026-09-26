import { useEffect, useMemo, useRef, useState } from "react";
import type { ProjectDialogPort } from "../application/projectDialogPort";
import { createProjectDecisions } from "../application/projectDecision";
import type { AlbumInformationImpact, EdgeConversionLoss } from "../domain/project";
import type { AlbumInformationProjectDraft } from "../application/projectSettingsDraft";
import { createAlbumInformationReview, type AlbumInformationCommitResult, type AlbumInformationReview } from "../application/albumInformationReview";

/** Commits whose review keeps changing without a consequence are not retried forever. */
const MAX_SILENT_APPLIES = 3;

interface AlbumInformationApplyControllerOptions {
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
        let review = createAlbumInformationReview(draft.baseline, draft.value, impact);
        let silentApplies = 0;
        while (decision.current) {
          // The changed values are visible in the panel and one Undo reverts them;
          // only a consequence the panel cannot show asks for confirmation.
          const consequences = albumInformationConsequences(review);
          if (consequences.length > 0) {
            const confirmed = await decision.ask({ kind: "albumInformationConfirmation", busy: false, consequences },
              (action) => action === "confirmAlbumInformation" ? true : action === "cancelAlbumInformation" ? false : undefined);
            if (!confirmed || !await decision.present({ kind: "albumInformationConfirmation", busy: true, consequences })) return false;
          } else if (++silentApplies > MAX_SILENT_APPLIES) {
            return false;
          }
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

/**
 * Sentences for what applying does that the panel cannot show: customizations
 * removed when an end becomes a single page, and photos that may be recropped
 * when the sheet proportion changes. Empty when applying needs no confirmation.
 */
export function albumInformationConsequences(review: AlbumInformationReview): string[] {
  const consequences: string[] = [];
  if (review.conversionLosses.length > 0) {
    consequences.push(edgeConversionConsequence(review));
  }
  if (review.impact.dimensionalChange?.proportionChanged) {
    consequences.push("A proporção das lâminas muda. As fotos mantêm a proporção, e o recorte pode ser ajustado.");
  }
  return consequences;
}

function edgeConversionConsequence({ baseline, information, conversionLosses }: AlbumInformationReview) {
  const firstBecomesSingle = baseline.firstSheet === "double" && information.firstSheet === "singlePage";
  const lastBecomesSingle = baseline.lastSheet === "double" && information.lastSheet === "singlePage";
  const ends = firstBecomesSingle && lastBecomesSingle ? "A primeira e a última lâmina viram página única."
    : firstBecomesSingle ? "A primeira lâmina vira página única."
      : lastBecomesSingle ? "A última lâmina vira página única." : null;
  const removed = conversionLosses.map(lostCustomization);
  const plural = removed.length > 1 || conversionLosses.some((loss) => loss.background && loss.overlay);
  const verb = plural ? "serão removidos"
    : conversionLosses[0]?.background ? "será removido" : "será removida";
  const list = removed.length > 1 ? `${removed.slice(0, -1).join(", ")} e ${removed[removed.length - 1]}` : removed[0];
  const removal = `${list[0].toLocaleUpperCase("pt-BR")}${list.slice(1)} ${verb}.`;
  return ends ? `${ends} ${removal}` : removal;
}

function lostCustomization(loss: EdgeConversionLoss) {
  const subject = loss.background && loss.overlay ? "o fundo e a sobreposição"
    : loss.background ? "o fundo" : "a sobreposição";
  return `${subject} da lâmina ${loss.sheetNumber}`;
}

function messageFromError(error: unknown) {
  return error instanceof Error
    ? error.message
    : "Não foi possível abrir a confirmação das Informações do álbum.";
}
