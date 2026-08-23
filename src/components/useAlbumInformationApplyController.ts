import { useCallback, useEffect, useRef, useState } from "react";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
} from "../application/projectDialogPort";
import type {
  AlbumInformation,
  AlbumInformationImpact,
} from "../domain/project";
import { formatPhysicalMeasurement } from "../application/physicalMeasurements";

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
  const details = [
    `Lâmina: ${measurement(information.sheetWidthUm)} × ${measurement(information.sheetHeightUm)}`,
    `Resolução final: Lâmina ${formatPixels(impact.sheetWidthPx)} × ${formatPixels(impact.heightPx)} px · Página ${formatPixels(impact.pageWidthPx)} × ${formatPixels(impact.heightPx)} px`,
    `DPI: ${information.dpi}`,
    `Sangria: ${measurement(information.bleedUm)} · Segurança: ${measurement(information.safetyUm)}`,
  ];
  if (
    information.sheetWidthUm !== baseline.sheetWidthUm ||
    information.sheetHeightUm !== baseline.sheetHeightUm
  ) {
    details.push(
      "Dimensão: a proporção da composição será preservada no novo formato.",
    );
  }
  if (
    information.firstSheet !== baseline.firstSheet ||
    information.lastSheet !== baseline.lastSheet
  ) {
    details.push(
      `Extremidades: ${formatEnd(baseline.firstSheet)} / ${formatEnd(baseline.lastSheet)} → ${formatEnd(information.firstSheet)} / ${formatEnd(information.lastSheet)}`,
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
