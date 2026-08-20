import {
  useEffect,
  useMemo,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import { X } from "lucide-react";

import type {
  DocumentSnapshot,
  MediaCatalogItem,
  ProjectedBackgroundContent,
  ProjectedOverlayContent,
  ProjectedVisualDefaults,
} from "../domain/project";
import type { NewProjectPersonalizationDraft } from "../global/application/newProjectPersonalization";
import type { NewProjectPreviewGeometry } from "../global/newProjectPreviewGeometry";
import { PersonalizationScopeSurface } from "../global/PersonalizationScopeSurface";
import { ProportionalPreviewViewport } from "../global/ProportionalPreviewViewport";
import { ActionButton, AppIcon } from "../ui";
import { AlbumFrameBorderPreview } from "./AlbumFrameBorderPreview";
import {
  setAlbumBackground,
  setAlbumFrameBorder,
  setAlbumOverlay,
  type AlbumDesignScope,
} from "./albumDesignDraft";
import { micrometersToDisplayUnits } from "./measurementFormatting";

const FRAME_BORDER_COLORS = ["#FFFFFF", "#2C2924", "#C5A46D"] as const;
const DEFAULT_FRAME_BORDER = { rgb: "#2C2924", widthUm: 1_000 };

interface AlbumDesignFormProps {
  document: DocumentSnapshot;
  formId: string;
  mediaItems: readonly MediaCatalogItem[];
  mediaPreviewUrls: Readonly<Record<string, string>>;
  value: ProjectedVisualDefaults;
  onApply(value: ProjectedVisualDefaults): void | Promise<unknown>;
  onReadyChange(ready: boolean): void;
}

export function AlbumDesignForm({
  document,
  formId,
  mediaItems,
  mediaPreviewUrls,
  value,
  onApply,
  onReadyChange,
}: AlbumDesignFormProps) {
  const baselineSignature = JSON.stringify(value);
  const [draft, setDraft] = useState(value);
  const [scope, setScope] = useState<AlbumDesignScope>("both");
  const [focusedScope, setFocusedScope] = useState<
    NewProjectPersonalizationDraft["fixedScope"] | null
  >(null);
  const [hoveredScope, setHoveredScope] = useState<
    NewProjectPersonalizationDraft["fixedScope"] | null
  >(null);
  const [openPicker, setOpenPicker] = useState<"background" | "overlay" | null>(
    null,
  );
  const [borderEditor, setBorderEditor] = useState(() =>
    value.frameBorder.kind === "solid"
      ? { rgb: value.frameBorder.rgb, widthUm: value.frameBorder.widthUm }
      : DEFAULT_FRAME_BORDER,
  );
  const decorativeMedia = useMemo(
    () => mediaItems.filter((media) => media.kind === "decorative"),
    [mediaItems],
  );
  const dirty = JSON.stringify(draft) !== baselineSignature;
  const background = backgroundAtScope(draft, scope);
  const overlay = overlayAtScope(draft, scope);
  const borderEnabled = draft.frameBorder.kind === "solid";
  const previewPersonalization = albumDesignPreviewDraft(
    draft,
    scope,
    mediaItems,
    mediaPreviewUrls,
  );
  const previewGeometry: NewProjectPreviewGeometry = {
    bleedUm: document.bleedUm,
    heightUm: document.sheetHeightUm,
    safetyUm: document.safetyUm,
    widthUm: document.sheetWidthUm,
  };

  useEffect(() => {
    setDraft(value);
    setBorderEditor(
      value.frameBorder.kind === "solid"
        ? { rgb: value.frameBorder.rgb, widthUm: value.frameBorder.widthUm }
        : DEFAULT_FRAME_BORDER,
    );
  }, [baselineSignature, value]);

  useEffect(() => onReadyChange(dirty), [dirty, onReadyChange]);

  function chooseBackground(content: ProjectedBackgroundContent) {
    setDraft((current) => setAlbumBackground(current, scope, content));
    setOpenPicker(null);
  }

  function chooseOverlay(content: ProjectedOverlayContent | null) {
    setDraft((current) => setAlbumOverlay(current, scope, content));
    setOpenPicker(null);
  }

  function updateBorder(next: typeof borderEditor) {
    setBorderEditor(next);
    if (borderEnabled) {
      setDraft((current) =>
        setAlbumFrameBorder(current, { kind: "solid", ...next }),
      );
    }
  }

  return (
    <form
      id={formId}
      className="inspector-subsections album-design-form"
      onSubmit={(event) => {
        event.preventDefault();
        if (dirty) void onApply(draft);
      }}
    >
      <section className="inspector-subsection">
        <h3>Padrões visuais</h3>
        <div className="album-design-preview">
          <ProportionalPreviewViewport
            height={previewGeometry.heightUm}
            label="Prévia do padrão visual do Álbum"
            width={previewGeometry.widthUm}
          >
            <PersonalizationScopeSurface
              includeBothSidesControl
              focusedScope={focusedScope}
              frameGapUm={6_000}
              geometry={previewGeometry}
              hoveredScope={hoveredScope}
              personalization={previewPersonalization}
              onFocusedScopeChange={setFocusedScope}
              onHoveredScopeChange={setHoveredScope}
              onScopeChange={setScope}
            />
          </ProportionalPreviewViewport>
        </div>
        <p className="ui-section-eyebrow album-design-scope-label">
          {scopeLabel(scope)}
        </p>
        <VisualDefaultControl
          currentLabel={backgroundLabel(background)}
          currentPreview={backgroundPreview(background, mediaPreviewUrls)}
          decorativeMedia={decorativeMedia}
          label="Background"
          mediaPreviewUrls={mediaPreviewUrls}
          open={openPicker === "background"}
          onOpenChange={(open) => setOpenPicker(open ? "background" : null)}
          onSelect={(mediaId) =>
            chooseBackground({ kind: "media", mediaId })
          }
        >
          <label className="visual-default-picker__color">
            <span className="ui-visually-hidden">Cor do Background</span>
            <input
              aria-label="Cor do Background"
              type="color"
              value={backgroundColor(background)}
              onChange={(event) =>
                chooseBackground({
                  kind: "color",
                  rgb: event.currentTarget.value.toUpperCase(),
                })
              }
            />
          </label>
        </VisualDefaultControl>
        <VisualDefaultControl
          currentLabel={overlayLabel(overlay)}
          currentPreview={overlayPreview(overlay, mediaPreviewUrls)}
          decorativeMedia={decorativeMedia}
          label="Overlay"
          mediaPreviewUrls={mediaPreviewUrls}
          open={openPicker === "overlay"}
          onOpenChange={(open) => setOpenPicker(open ? "overlay" : null)}
          onSelect={(mediaId) => chooseOverlay({ kind: "media", mediaId })}
          onClear={() => chooseOverlay(null)}
        />
      </section>
      <section className="inspector-subsection">
        <h3>Padrão dos Frames</h3>
        <AlbumFrameBorderPreview frameBorder={draft.frameBorder} />
        <label className="album-frame-border-toggle">
          <input
            aria-label="Exibir borda"
            checked={borderEnabled}
            type="checkbox"
            onChange={(event) => {
              const enabled = event.currentTarget.checked;
              setDraft((current) =>
                setAlbumFrameBorder(
                  current,
                  enabled
                    ? { kind: "solid", ...borderEditor }
                    : { kind: "none" },
                ),
              );
            }}
          />
          <span aria-hidden="true" className="album-frame-border-toggle__track" />
          <span>Exibir borda</span>
          <output>{borderEnabled ? "com borda" : "sem borda"}</output>
        </label>
        {borderEnabled ? (
          <>
            <div
              aria-label="Cores da Borda"
              className="album-frame-border-colors"
              role="group"
            >
              {FRAME_BORDER_COLORS.map((color) => (
                <button
                  aria-label={`Usar cor da Borda ${color}`}
                  aria-pressed={borderEditor.rgb === color}
                  key={color}
                  style={{ background: color }}
                  type="button"
                  onClick={() => updateBorder({ ...borderEditor, rgb: color })}
                />
              ))}
              <label className="album-frame-border-color-picker">
                <span className="ui-visually-hidden">Cor da Borda</span>
                <input
                  aria-label="Cor da Borda"
                  type="color"
                  value={borderEditor.rgb}
                  onChange={(event) =>
                    updateBorder({
                      ...borderEditor,
                      rgb: event.currentTarget.value.toUpperCase(),
                    })
                  }
                />
              </label>
            </div>
            <label className="album-frame-border-range">
              <span>
                <span>Espessura da Borda</span>
                <output>
                  {formatMeasurement(borderEditor.widthUm, document.displayUnit)}
                </output>
              </span>
              <input
                aria-label="Espessura da Borda"
                max={Math.max(5_000, borderEditor.widthUm)}
                min="250"
                step="250"
                type="range"
                value={borderEditor.widthUm}
                onChange={(event) =>
                  updateBorder({
                    ...borderEditor,
                    widthUm: Number(event.currentTarget.value),
                  })
                }
              />
            </label>
          </>
        ) : null}
      </section>
    </form>
  );
}

function VisualDefaultControl({
  children,
  currentLabel,
  currentPreview,
  decorativeMedia,
  label,
  mediaPreviewUrls,
  open,
  onClear,
  onOpenChange,
  onSelect,
}: {
  children?: ReactNode;
  currentLabel: string;
  currentPreview: CSSProperties;
  decorativeMedia: readonly MediaCatalogItem[];
  label: "Background" | "Overlay";
  mediaPreviewUrls: Readonly<Record<string, string>>;
  open: boolean;
  onClear?: () => void;
  onOpenChange(open: boolean): void;
  onSelect(mediaId: string): void;
}) {
  const recent = decorativeMedia.slice(0, 2);
  return (
    <div className="visual-default-field">
      <span className="album-design-label">{label}</span>
      <div className="visual-default-picker">
        <button
          aria-expanded={open}
          aria-haspopup="dialog"
          aria-label={`Escolher ${label}`}
          className="visual-default-picker__current"
          type="button"
          onClick={() => onOpenChange(!open)}
        >
          <span
            aria-hidden="true"
            className={`visual-default-picker__preview ${
              currentLabel === "sem" ? "visual-default-picker__preview--none" : ""
            }`}
            style={currentPreview}
          />
          <span>{currentLabel}</span>
        </button>
        <span aria-hidden="true" className="visual-default-picker__divider" />
        <div className="visual-default-picker__recent">
          <div className="visual-default-picker__tiles">
            {children}
            {recent.map((media) => (
              <button
                aria-label={`Usar ${label} ${media.name}`}
                className="visual-default-picker__tile"
                key={media.id}
                style={decorativePreview(media, mediaPreviewUrls)}
                type="button"
                onClick={() => onSelect(media.id)}
              />
            ))}
            <button
              aria-label={`Abrir mais opções de ${label}`}
              className="visual-default-picker__add"
              type="button"
              onClick={() => onOpenChange(!open)}
            >
              +
            </button>
          </div>
          <span>{recent.length ? "Decorativos recentes" : "Sem Decorativos"}</span>
        </div>
      </div>
      {open ? (
        <div
          aria-label={`Escolher ${label}`}
          className="album-design-decorative-picker"
          role="dialog"
        >
          {onClear ? (
            <ActionButton density="compact" variant="quiet" onClick={onClear}>
              <AppIcon icon={X} size={12} />
              Sem Overlay
            </ActionButton>
          ) : null}
          {decorativeMedia.length ? (
            decorativeMedia.map((media) => (
              <button
                aria-label={`Selecionar ${label} ${media.name}`}
                className="album-design-decorative-option"
                key={media.id}
                type="button"
                onClick={() => onSelect(media.id)}
              >
                <span
                  aria-hidden="true"
                  style={decorativePreview(media, mediaPreviewUrls)}
                />
                <span>{media.name}</span>
              </button>
            ))
          ) : (
            <p>Nenhum Decorativo importado.</p>
          )}
          <ActionButton
            aria-label={`Fechar seletor de ${label}`}
            density="compact"
            variant="quiet"
            onClick={() => onOpenChange(false)}
          >
            Fechar
          </ActionButton>
        </div>
      ) : null}
    </div>
  );
}

function albumDesignPreviewDraft(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
  mediaItems: readonly MediaCatalogItem[],
  mediaPreviewUrls: Readonly<Record<string, string>>,
): NewProjectPersonalizationDraft {
  const mediaById = new Map(mediaItems.map((media) => [media.id, media]));
  const selection = (mediaId: string) => ({
    selectionId: mediaId,
    displayName: mediaById.get(mediaId)?.name ?? "Decorativo",
    previewUrl: mediaPreviewUrls[mediaId] ?? "",
  });
  const backgroundContent = (content: ProjectedBackgroundContent) =>
    content.kind === "color"
      ? content
      : { kind: "image" as const, selection: selection(content.mediaId) };
  const overlayContent = (content: ProjectedOverlayContent | null) =>
    content
      ? { kind: "image" as const, selection: selection(content.mediaId) }
      : null;

  return {
    fixedScope: scope,
    background:
      defaults.background.scope === "bothSides"
        ? {
            scope: "bothSides",
            both: backgroundContent(defaults.background.both),
          }
        : {
            scope: "perSide",
            left: backgroundContent(defaults.background.left),
            right: backgroundContent(defaults.background.right),
          },
    overlay:
      defaults.overlay.scope === "bothSides"
        ? {
            scope: "bothSides",
            both: overlayContent(defaults.overlay.both),
          }
        : {
            scope: "perSide",
            left: overlayContent(defaults.overlay.left),
            right: overlayContent(defaults.overlay.right),
          },
    frameBorder: defaults.frameBorder,
  };
}

function backgroundAtScope(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
) {
  const { background } = defaults;
  if (scope === "both") {
    if (background.scope === "bothSides") return background.both;
    return sameBackground(background.left, background.right)
      ? background.left
      : null;
  }
  return background.scope === "bothSides" ? background.both : background[scope];
}

function overlayAtScope(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
) {
  const { overlay } = defaults;
  if (scope === "both") {
    if (overlay.scope === "bothSides") return overlay.both;
    return sameOverlay(overlay.left, overlay.right) ? overlay.left : undefined;
  }
  return overlay.scope === "bothSides" ? overlay.both : overlay[scope];
}

function sameBackground(
  left: ProjectedBackgroundContent,
  right: ProjectedBackgroundContent,
) {
  if (left.kind !== right.kind) return false;
  return left.kind === "color"
    ? right.kind === "color" && left.rgb === right.rgb
    : right.kind === "media" && left.mediaId === right.mediaId;
}

function sameOverlay(
  left: ProjectedOverlayContent | null,
  right: ProjectedOverlayContent | null,
) {
  if (left === null || right === null) return left === right;
  return left.mediaId === right.mediaId;
}

function backgroundColor(content: ProjectedBackgroundContent | null) {
  return content?.kind === "color" ? content.rgb : "#FFFFFF";
}

function backgroundLabel(content: ProjectedBackgroundContent | null) {
  if (content === null) return "por lado";
  return content.kind === "color" ? "cor" : "imagem";
}

function overlayLabel(content: ProjectedOverlayContent | null | undefined) {
  if (content === undefined) return "por lado";
  return content === null ? "sem" : "imagem";
}

function backgroundPreview(
  content: ProjectedBackgroundContent | null,
  mediaPreviewUrls: Readonly<Record<string, string>>,
): CSSProperties {
  if (content === null) return {};
  if (content.kind === "color") return { background: content.rgb };
  return mediaPreview(content.mediaId, mediaPreviewUrls);
}

function overlayPreview(
  content: ProjectedOverlayContent | null | undefined,
  mediaPreviewUrls: Readonly<Record<string, string>>,
): CSSProperties {
  return content ? mediaPreview(content.mediaId, mediaPreviewUrls) : {};
}

function decorativePreview(
  media: MediaCatalogItem,
  mediaPreviewUrls: Readonly<Record<string, string>>,
): CSSProperties {
  return mediaPreviewUrls[media.id]
    ? mediaPreview(media.id, mediaPreviewUrls)
    : { background: media.palette?.[1] ?? "var(--ui-surface-muted)" };
}

function mediaPreview(
  mediaId: string,
  mediaPreviewUrls: Readonly<Record<string, string>>,
): CSSProperties {
  const url = mediaPreviewUrls[mediaId];
  return url
    ? { backgroundImage: `url("${url}")`, backgroundPosition: "center", backgroundSize: "cover" }
    : {};
}

function scopeLabel(scope: AlbumDesignScope) {
  if (scope === "left") return "Página esquerda";
  if (scope === "right") return "Página direita";
  return "Ambos os lados";
}

const measurementFormatter = new Intl.NumberFormat("pt-BR", {
  maximumFractionDigits: 6,
  useGrouping: false,
});

function formatMeasurement(
  micrometers: number,
  unit: DocumentSnapshot["displayUnit"],
) {
  return `${measurementFormatter.format(
    micrometersToDisplayUnits(micrometers, unit),
  )} ${unit}`;
}
