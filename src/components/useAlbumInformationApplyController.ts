import { useCallback, useEffect, useRef, useState } from "react";

import type {
  ProjectDialogDetail,
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogSession,
} from "../application/projectDialogPort";
import type {
  AlbumInformation,
  AlbumInformationImpact,
} from "../domain/project";
import type { AlbumInformationProjectDraft } from "../application/projectSettingsDraft";
import {
  createAlbumInformationReview,
  type AlbumInformationCommitResult,
  type AlbumInformationReview,
} from "../application/albumInformationReview";
import {
  displayUnitLabel,
  formatPhysicalMeasurement,
} from "../application/physicalMeasurements";

interface AlbumInformationApplyControllerOptions {
  projectDialogPort: ProjectDialogPort;
  onApply(
    draft: AlbumInformationProjectDraft,
    confirmedReview: AlbumInformationReview,
  ): Promise<AlbumInformationCommitResult>;
  onError(message: string): void;
}

type Phase = "idle" | "deciding" | "applying" | "closing";

interface PendingAlbumInformation {
  draft: AlbumInformationProjectDraft;
  review: AlbumInformationReview;
}

interface ApplyCompletion {
  resolve(completed: boolean): void;
}

export function useAlbumInformationApplyController({
  projectDialogPort,
  onApply,
  onError,
}: AlbumInformationApplyControllerOptions) {
  const [active, setActive] = useState(false);
  const phaseRef = useRef<Phase>("idle");
  const pendingRef = useRef<PendingAlbumInformation | null>(null);
  const completionRef = useRef<ApplyCompletion | null>(null);
  const dialogSessionRef = useRef<ProjectDialogSession | null>(null);
  const actionListenerRef = useRef<(action: ProjectDialogAction) => void>(
    () => undefined,
  );

  const finish = useCallback(async (completed: boolean) => {
    phaseRef.current = "closing";
    pendingRef.current = null;
    const completion = completionRef.current;
    completionRef.current = null;
    const session = dialogSessionRef.current;
    dialogSessionRef.current = null;
    await session?.dismiss().catch(() => undefined);
    phaseRef.current = "idle";
    setActive(false);
    completion?.resolve(completed);
  }, []);

  const requestApply = useCallback(
    async (
      draft: AlbumInformationProjectDraft,
      impact: AlbumInformationImpact,
    ) => {
      if (phaseRef.current !== "idle") return false;
      let resolveCompletion!: (completed: boolean) => void;
      const completion = new Promise<boolean>((resolve) => {
        resolveCompletion = resolve;
      });
      completionRef.current = { resolve: resolveCompletion };
      phaseRef.current = "deciding";
      const review = createAlbumInformationReview(
        draft.baseline,
        draft.value,
        impact,
      );
      pendingRef.current = { draft, review };
      setActive(true);
      const session = projectDialogPort.acquire(
        (action) => actionListenerRef.current(action),
      );
      dialogSessionRef.current = session;
      try {
        await session.present({
          busy: false,
          details: detailsFromReview(review),
          kind: "albumInformationConfirmation",
        });
      } catch (error: unknown) {
        if (dialogSessionRef.current !== session) return completion;
        dialogSessionRef.current = null;
        void session.dismiss().catch(() => undefined);
        phaseRef.current = "idle";
        pendingRef.current = null;
        setActive(false);
        completionRef.current = null;
        resolveCompletion(false);
        onError(messageFromError(error));
      }
      return completion;
    },
    [onError, projectDialogPort],
  );

  const confirm = useCallback(async () => {
    const pending = pendingRef.current;
    if (phaseRef.current !== "deciding" || !pending) return;
    phaseRef.current = "applying";
    const session = dialogSessionRef.current;
    if (!session) {
      await finish(false);
      return;
    }
    try {
      await session.present({
        busy: true,
        details: detailsFromReview(pending.review),
        kind: "albumInformationConfirmation",
      });
    } catch (error: unknown) {
      if (dialogSessionRef.current !== session) return;
      onError(messageFromError(error));
      await finish(false);
      return;
    }
    if (
      dialogSessionRef.current !== session ||
      phaseRef.current !== "applying" ||
      pendingRef.current !== pending
    ) {
      return;
    }
    let result: AlbumInformationCommitResult;
    try {
      result = await onApply(pending.draft, pending.review);
    } catch (error: unknown) {
      onError(messageFromError(error));
      await finish(false);
      return;
    }
    if (result.kind === "reviewRequired") {
      pendingRef.current = { draft: pending.draft, review: result.review };
      phaseRef.current = "deciding";
      try {
        await dialogSessionRef.current?.present({
          busy: false,
          details: detailsFromReview(result.review),
          kind: "albumInformationConfirmation",
        });
      } catch (error: unknown) {
        onError(messageFromError(error));
        await finish(false);
      }
      return;
    }
    await finish(result.kind === "completed");
  }, [finish, onApply, onError]);

  actionListenerRef.current = (action) => {
    if (action === "cancelAlbumInformation" && phaseRef.current === "deciding") {
      void finish(false);
    }
    if (action === "confirmAlbumInformation") {
      void confirm();
    }
  };

  useEffect(
    () => () => {
      const session = dialogSessionRef.current;
      dialogSessionRef.current = null;
      completionRef.current?.resolve(false);
      completionRef.current = null;
      void session?.dismiss().catch(() => undefined);
    },
    [],
  );

  return { active, requestApply };
}

function detailsFromReview(review: AlbumInformationReview) {
  return albumInformationDetails(
    review.information,
    review.baseline,
    review.impact,
  );
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
      value: "A proporção será preservada no novo formato.",
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
