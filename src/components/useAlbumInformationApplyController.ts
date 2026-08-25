import { useCallback, useEffect, useRef, useState } from "react";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
} from "../application/projectDialogPort";
import type {
  AlbumInformation,
  AlbumInformationImpact,
} from "../domain/project";
import {
  displayUnitLabel,
  formatPhysicalMeasurement,
} from "../application/physicalMeasurements";

interface AlbumInformationApplyControllerOptions {
  projectDialogPort: ProjectDialogPort;
  onApply(information: AlbumInformation): Promise<boolean>;
  onError(message: string): void;
}

type Phase = "idle" | "deciding" | "applying";

interface PendingAlbumInformation {
  baseline: AlbumInformation;
  impact: AlbumInformationImpact;
  information: AlbumInformation;
}

export function useAlbumInformationApplyController({
  projectDialogPort,
  onApply,
  onError,
}: AlbumInformationApplyControllerOptions) {
  const [active, setActive] = useState(false);
  const phaseRef = useRef<Phase>("idle");
  const pendingRef = useRef<PendingAlbumInformation | null>(null);
  const actionListenerRef = useRef<(action: ProjectDialogAction) => void>(
    () => undefined,
  );

  const finish = useCallback(() => {
    phaseRef.current = "idle";
    pendingRef.current = null;
    setActive(false);
    void projectDialogPort.dismiss().catch(() => undefined);
  }, [projectDialogPort]);

  const requestApply = useCallback(
    async (
      information: AlbumInformation,
      baseline: AlbumInformation,
      impact: AlbumInformationImpact,
    ) => {
      if (phaseRef.current !== "idle") return;
      phaseRef.current = "deciding";
      pendingRef.current = { baseline, impact, information };
      setActive(true);
      try {
        await projectDialogPort.present({
          busy: false,
          details: albumInformationDetails(information, baseline, impact),
          kind: "albumInformationConfirmation",
        });
      } catch (error: unknown) {
        phaseRef.current = "idle";
        pendingRef.current = null;
        setActive(false);
        onError(messageFromError(error));
      }
    },
    [onError, projectDialogPort],
  );

  const confirm = useCallback(async () => {
    const pending = pendingRef.current;
    if (phaseRef.current !== "deciding" || !pending) return;
    phaseRef.current = "applying";
    void projectDialogPort
      .present({
        busy: true,
        details: albumInformationDetails(
          pending.information,
          pending.baseline,
          pending.impact,
        ),
        kind: "albumInformationConfirmation",
      })
      .catch(() => undefined);
    try {
      await onApply(pending.information);
    } finally {
      finish();
    }
  }, [finish, onApply, projectDialogPort]);

  actionListenerRef.current = (action) => {
    if (action === "cancelAlbumInformation" && phaseRef.current === "deciding") {
      finish();
    }
    if (action === "confirmAlbumInformation") {
      void confirm();
    }
  };

  useEffect(() => {
    let current = true;
    let unsubscribe: (() => void) | undefined;
    void projectDialogPort
      .onAction((action) => actionListenerRef.current(action))
      .then((registeredUnsubscribe) => {
        if (current) unsubscribe = registeredUnsubscribe;
        else registeredUnsubscribe();
      })
      .catch((error: unknown) => {
        if (current) onError(messageFromError(error));
      });
    return () => {
      current = false;
      unsubscribe?.();
    };
  }, [onError, projectDialogPort]);

  return { active, requestApply };
}

export function albumInformationDetails(
  information: AlbumInformation,
  baseline: AlbumInformation,
  impact: AlbumInformationImpact,
) {
  const measurement = (valueUm: number) =>
    formatPhysicalMeasurement(valueUm, information.displayUnit);
  const formatEnd = (value: AlbumInformation["firstSheet"]) =>
    value === "double" ? "Lâmina dupla" : "Página única";
  const details: string[] = [];
  const addChange = (
    label: string,
    before: string | number,
    after: string | number,
  ) => details.push(`${label}: ${before} → ${after}`);
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
    details.push(
      `Resolução resultante: Lâmina ${formatPixels(impact.sheetWidthPx)} × ${formatPixels(impact.heightPx)} px · Página ${formatPixels(impact.pageWidthPx)} × ${formatPixels(impact.heightPx)} px`,
    );
  }
  if (dimensionsChanged) {
    details.push(
      "Composição: A proporção será preservada no novo formato.",
    );
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
