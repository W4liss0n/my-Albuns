import { useEffect, useRef, useState } from "react";
import { LockKeyhole, LockKeyholeOpen, Star, Trash2 } from "lucide-react";
import type { ComposedFrame, ComposedSheet } from "../domain/project";
import { AppIcon } from "../ui";
import { useDismissableSurface } from "../ui/useDismissableSurface";
import { SheetPreviewSurface } from "./SheetPreview";
import type { LayoutPanelController } from "./useLayoutPanel";
import type { LayoutCatalogController } from "./useLayoutCatalog";
import "./LayoutPanel.css";

interface LayoutPanelProps {
  controller: LayoutPanelController;
  sheet: ComposedSheet;
  catalog?: LayoutCatalogController;
}

export function LayoutPanel({ controller, sheet, catalog }: LayoutPanelProps) {
  const rootRef = useRef<HTMLElement>(null);
  const [highlightId, setHighlightId] = useState<string | null>(null);
  const revealId = catalog?.revealId;
  const acknowledgeReveal = useRef(catalog?.acknowledgeReveal);
  acknowledgeReveal.current = catalog?.acknowledgeReveal;
  useEffect(() => {
    if (!revealId || !controller.query?.listing.candidates.some((candidate) => candidate.customId === revealId)) return;
    const card = rootRef.current?.querySelector(`[data-custom-layout-id="${revealId}"]`);
    if (!card) return;
    card.scrollIntoView?.({ block: "nearest", inline: "center" });
    setHighlightId(revealId);
    acknowledgeReveal.current?.();
  }, [revealId, controller.query]);
  useEffect(() => {
    if (!highlightId) return;
    const timeout = window.setTimeout(() => setHighlightId(null), 3000);
    return () => window.clearTimeout(timeout);
  }, [highlightId]);
  useDismissableSurface({ enabled: true, capturePointerOutside: true, rootRef,
    onDismiss({ reason, event }) {
      if (reason !== "pointerOutside") return;
      // A Sheet bar owns toggling or redirecting the panel; closing on its
      // pointerdown would make the subsequent click reopen the same target.
      if (event.target instanceof Element && event.target.closest('[aria-controls="layout-panel"]')) return;
      controller.close();
    } });
  const query = controller.displayQuery;
  const busy = controller.committing || controller.query === null;
  const count = sheet.frames.length;
  const minimumCount = controller.minimumPositionCount;
  const counts = Array.from({ length: Math.max(0, 31 - minimumCount) }, (_, index) => minimumCount + index);
  if (controller.positionCount > 30) counts.push(controller.positionCount);
  const emptyMessage = query?.listing.generationStatus === "empty"
    ? "Escolha a quantidade de Frames para preparar um Layout."
    : query?.listing.generationStatus === "outsideCoverage"
      ? "As sugestões automáticas atendem de 1 a 30 Frames."
      : "Nenhuma sugestão atende às medidas atuais.";
  return (
    <section aria-label="Painel de Layouts" className="layout-panel" id="layout-panel" ref={rootRef}>
      <div className="layout-panel__header">
        <label className="layout-panel__positions">Frames
          <select aria-label="Quantidade de Frames" value={controller.positionCount}
            disabled={controller.committing || query?.locked || minimumCount > 30}
            onChange={(event) => controller.configurePositions(Number(event.target.value))}>
            {counts.map((value) =>
              <option key={value} value={value}>{value}</option>)}
          </select>
        </label>
      </div>
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
              return <div className={`layout-panel__candidate${locked ? " layout-panel__candidate--locked" : ""}${unavailable ? " layout-panel__candidate--unavailable" : ""}${candidate.customId && candidate.customId === highlightId ? " layout-panel__candidate--revealed" : ""}`}
                data-custom-layout-id={candidate.customId ?? undefined}
                key={JSON.stringify(candidate.layout)} onPointerEnter={() => controller.preview(index)}>
              <button
              aria-label={`Aplicar Layout ${index + 1}${candidate.isLastApplied ? " — último aplicado" : ""}`}
              className="layout-panel__preview" disabled={busy || query!.locked || extraPositions > 0} type="button"
              onFocus={() => controller.preview(index)} onBlur={controller.cancelPreview}
              onClick={() => { void controller.apply(index); }}
              title={extraPositions > 0 ? "Use o cadeado para aplicar e criar as posições adicionais." : `${candidate.layout.definition.scope === "page" ? "Por Página" : "Por Lâmina"}${candidate.isLastApplied ? " · Último aplicado" : ""}`}>
              <LayoutThumbnail sheet={sheet} frames={controller.previews[index]} />
              </button>
              <button className="layout-panel__lock layout-panel__favorite" type="button" disabled={busy || unavailable}
                aria-label={`${candidate.favoriteId ? "Remover dos favoritos" : "Favoritar"} Layout ${index + 1}`}
                aria-pressed={candidate.favoriteId !== null}
                title={candidate.favoriteId ? "Remover dos favoritos deste Projeto" : "Favoritar neste Projeto"}
                onClick={() => { void controller.toggleFavorite(index); }}>
                <AppIcon icon={Star} size={12} />
              </button>
              <button className="layout-panel__lock" type="button" disabled={busy || unavailable}
                aria-label={locked ? `Destravar Layout da Lâmina ${String(sheet.number).padStart(2, "0")}` : `Aplicar e travar Layout ${index + 1}`}
                title={locked ? "Destravar Layout" : "Aplicar e travar Layout"}
                onFocus={() => controller.preview(index)} onBlur={controller.cancelPreview}
                onClick={() => { void (locked ? controller.unlock() : controller.lock(index)); }}>
                <AppIcon icon={locked ? LockKeyhole : LockKeyholeOpen} size={12} />
              </button>
              {catalog && candidate.customId && <button className="layout-panel__lock layout-panel__delete" type="button"
                disabled={busy || catalog.busy} aria-label={`Excluir Layout personalizado ${index + 1}`} title="Excluir Layout personalizado"
                onClick={() => { controller.cancelPreview(); catalog.requestDelete(candidate.customId!); }}>
                <AppIcon icon={Trash2} size={12} />
              </button>}
            </div>; })}
            {candidates.length === 0 && <p role="status">{controller.error ?? (origin === "custom" && query ? "Nenhum Layout personalizado."
              : query ? emptyMessage : "Consultando Layouts…")}</p>}
          </div>
        </div>;
      })}
    </section>
  );
}

function LayoutThumbnail({ sheet, frames }: { sheet: ComposedSheet; frames: ComposedFrame[] }) {
  return <SheetPreviewSurface activeSides={sheet.activeSides}>
    <svg aria-label={`Layout com ${frames.length} Frames`} className="sheet-preview layout-panel__geometry"
      focusable="false" role="img" preserveAspectRatio="xMidYMid meet"
      viewBox={`0 0 ${sheet.widthUm} ${sheet.heightUm}`}>
      <rect className="layout-panel__surface" width={sheet.widthUm} height={sheet.heightUm} />
      {frames.map((frame) => <rect className="layout-panel__frame" data-preview-frame-id={frame.frameId}
        key={frame.frameId} x={frame.clipRect.x} y={frame.clipRect.y}
        width={frame.clipRect.width} height={frame.clipRect.height} />)}
      {sheet.activeSides === "both" && <line className="layout-panel__center-line" vectorEffect="non-scaling-stroke"
        x1={sheet.widthUm / 2} x2={sheet.widthUm / 2} y1={0} y2={sheet.heightUm} />}
    </svg>
  </SheetPreviewSurface>;
}
