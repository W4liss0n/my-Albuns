import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import {
  changeFrameBorderColor,
  changeFrameBorderWidth,
  createFrameBorderEditorState,
} from "../application/frameBorderEditor";
import { formatPhysicalMeasurement } from "../application/physicalMeasurements";
import {
  mapScopedValue,
  readScopedValue,
} from "../application/scopedValues";
import {
  createAlbumDesignProjectDraft,
  type AlbumDesignProjectDraft,
} from "../application/projectSettingsDraft";
import type {
  DisplayUnit,
  DocumentSnapshot,
  MediaCatalogItem,
  ProjectedBackgroundContent,
  ProjectedOverlayContent,
  ProjectedVisualDefaults,
} from "../domain/project";
import {
  PersonalizationScopeSurface,
  ProportionalPreviewViewport,
  type VisualPersonalizationPreview,
  type VisualPreviewGeometry,
} from "../ui/visualPreview";
import {
  setAlbumBackground,
  setAlbumFrameBorder,
  setAlbumOverlay,
  type AlbumDesignScope,
} from "./albumDesignDraft";
import { DecorativeMediaPicker } from "./DecorativeMediaPicker";
import { useSemanticBaseline } from "./useSemanticBaseline";
import "./AlbumDesignForm.css";
import "./VisualDefaultPicker.css";

const DEFAULT_FRAME_BORDER = { rgb: "#2C2924", widthUm: 1_000 };

interface AlbumDesignFormProps {
  document: DocumentSnapshot;
  presentationUnit: DisplayUnit;
  formId: string;
  mediaItems: readonly MediaCatalogItem[];
  mediaPreviewUrls: Readonly<Record<string, string>>;
  revision: number;
  value: ProjectedVisualDefaults;
  onApply(draft: AlbumDesignProjectDraft): Promise<boolean>;
  onReadyChange(ready: boolean): void;
}

interface AlbumDesignDraftSession {
  current: AlbumDesignProjectDraft;
  pending: {
    submitted: AlbumDesignProjectDraft;
    subsequent: AlbumDesignProjectDraft;
  } | null;
}

export function AlbumDesignForm({
  document,
  presentationUnit,
  formId,
  mediaItems,
  mediaPreviewUrls,
  revision,
  value,
  onApply,
  onReadyChange,
}: AlbumDesignFormProps) {
  const baselineSignature = JSON.stringify(value);
  const baseline = useSemanticBaseline(
    { revision, value },
    baselineSignature,
  );
  const [draftSession, setDraftSession] = useState<AlbumDesignDraftSession>(
    () => ({
      current: createAlbumDesignProjectDraft(
        baseline.revision,
        baseline.value,
      ),
      pending: null,
    }),
  );
  const projectDraft = draftSession.current;
  const draft = projectDraft.value;
  const [scope, setScope] = useState<AlbumDesignScope>("both");
  const [borderEditor, setBorderEditor] = useState(() =>
    value.frameBorder.kind === "solid"
      ? { rgb: value.frameBorder.rgb, widthUm: value.frameBorder.widthUm }
      : DEFAULT_FRAME_BORDER,
  );
  const decorativeMedia = useMemo(
    () => mediaItems.filter((media) => media.kind === "decorative"),
    [mediaItems],
  );
  // PLACEHOLDER UI: o espaço entre Frames ainda não possui contrato de
  // persistência; a medida física controla somente a prévia desta seção.
  const [frameGapUm, setFrameGapUm] = useState(6_000);
  /**
   * Um seletor de Decorativo por vez. O estado vive aqui, e não em cada
   * controle, para que abrir um feche o outro por construção — inclusive
   * quando a abertura vem do teclado, que não emite `pointerdown`.
   */
  const [openPicker, setOpenPicker] = useState<
    "Background" | "Overlay" | null
  >(null);
  const [applying, setApplying] = useState(false);
  const dirty = projectDraft.changed;
  const ready = dirty && !applying;
  const background = backgroundAtScope(draft, scope);
  const overlay = overlayAtScope(draft, scope);
  const borderEnabled = draft.frameBorder.kind === "solid";
  const previewPersonalization = albumDesignPreviewDraft(
    draft,
    scope,
    mediaPreviewUrls,
  );
  const previewGeometry: VisualPreviewGeometry = {
    bleedUm: document.bleedUm,
    heightUm: document.sheetHeightUm,
    safetyUm: document.safetyUm,
    widthUm: document.sheetWidthUm,
  };

  useEffect(() => {
    setDraftSession((session) => {
      const { pending } = session;
      if (
        !pending ||
        pending.submitted.rebase(baseline.revision, baseline.value).changed
      ) {
        return {
          ...session,
          current: session.current.rebase(
            baseline.revision,
            baseline.value,
          ),
        };
      }
      return {
        current: pending.subsequent.rebase(
          baseline.revision,
          baseline.value,
        ),
        pending: null,
      };
    });
  }, [baseline]);

  useEffect(() => {
    if (draft.frameBorder.kind === "solid") {
      setBorderEditor({
        rgb: draft.frameBorder.rgb,
        widthUm: draft.frameBorder.widthUm,
      });
    }
  }, [draft.frameBorder]);

  useLayoutEffect(() => onReadyChange(ready), [onReadyChange, ready]);

  useLayoutEffect(
    () => () => {
      onReadyChange(false);
    },
    [onReadyChange],
  );

  function chooseBackground(content: ProjectedBackgroundContent) {
    transitionProjectDraft((current) =>
      setAlbumBackground(current, scope, content),
    );
  }

  function chooseOverlay(content: ProjectedOverlayContent | null) {
    transitionProjectDraft((current) =>
      setAlbumOverlay(current, scope, content),
    );
  }

  function updateBorder(next: ReturnType<typeof createFrameBorderEditorState>) {
    setBorderEditor(next.solid);
    transitionProjectDraft((current) =>
      setAlbumFrameBorder(current, next.border),
    );
  }

  function transitionProjectDraft(
    transition: (current: ProjectedVisualDefaults) => ProjectedVisualDefaults,
  ) {
    setDraftSession((session) => ({
      current: session.current.transition(
        transition(session.current.value),
      ),
      pending: session.pending
        ? {
            ...session.pending,
            subsequent: session.pending.subsequent.transition(
              transition(session.pending.subsequent.value),
            ),
          }
        : null,
    }));
  }

  /**
   * Espessura zero é a ausência de Borda, como na criação de novo Projeto: o
   * controle é o próprio slider, sem alternador separado.
   */
  function changeBorderWidth(widthUm: number) {
    updateBorder(
      changeFrameBorderWidth(
        createFrameBorderEditorState(draft.frameBorder, borderEditor),
        widthUm,
      ),
    );
  }

  async function submit() {
    if (!ready) return;
    const submitted = projectDraft;
    setDraftSession((session) => ({
      ...session,
      pending: {
        submitted,
        subsequent: createAlbumDesignProjectDraft(
          submitted.baselineRevision,
          submitted.value,
        ),
      },
    }));
    setApplying(true);
    let completed = false;
    try {
      completed = await onApply(submitted);
    } finally {
      if (!completed) {
        setDraftSession((session) =>
          session.pending?.submitted === submitted
            ? { current: session.current, pending: null }
            : session,
        );
      }
      setApplying(false);
    }
  }

  return (
    <form
      id={formId}
      className="inspector-subsections album-design-form"
      onSubmit={(event) => {
        event.preventDefault();
        void submit();
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
              focus={{ kind: "local" }}
              includeBothSidesControl
              frameGapUm={frameGapUm}
              geometry={previewGeometry}
              personalization={previewPersonalization}
              presentation={ALBUM_DESIGN_SCOPE_PRESENTATION}
              onScopeChange={setScope}
            />
          </ProportionalPreviewViewport>
        </div>
        <p className="ui-section-eyebrow album-design-scope-label">
          {scopeLabel(scope)}
        </p>
        <VisualDefaultControl
          decorativeMedia={decorativeMedia}
          label="Background"
          mediaPreviewUrls={mediaPreviewUrls}
          open={openPicker === "Background"}
          selectedMediaId={
            background?.kind === "media" ? background.mediaId : null
          }
          onOpenChange={(open) => setOpenPicker(open ? "Background" : null)}
          onSelect={(mediaId) => chooseBackground({ kind: "media", mediaId })}
        >
          <label
            className="visual-default-picker__option visual-default-picker__color"
            data-selected={background?.kind === "color" || undefined}
          >
            <span
              aria-hidden="true"
              className="visual-default-picker__tile"
              style={{ background: backgroundColor(background) }}
            />
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
          decorativeMedia={decorativeMedia}
          label="Overlay"
          mediaPreviewUrls={mediaPreviewUrls}
          noneSelected={overlay === null}
          open={openPicker === "Overlay"}
          selectedMediaId={overlay?.mediaId ?? null}
          onOpenChange={(open) => setOpenPicker(open ? "Overlay" : null)}
          onSelect={(mediaId) => chooseOverlay({ kind: "media", mediaId })}
          onClear={() => chooseOverlay(null)}
        />
      </section>
      <section className="inspector-subsection">
        <h3>Padrão dos Frames</h3>
        <div className="album-frame-border-row">
          <label className="album-frame-border-color-picker">
            <span className="ui-visually-hidden">Cor da Borda</span>
            <input
              aria-label="Cor da Borda"
              type="color"
              value={borderEditor.rgb}
              onChange={(event) =>
                updateBorder(
                  changeFrameBorderColor(
                    createFrameBorderEditorState(draft.frameBorder, borderEditor),
                    event.currentTarget.value,
                  ),
                )
              }
            />
          </label>
          <label className="ui-range-control">
            <span className="ui-range-control__heading">
              <span>Borda padrão</span>
              <output>
                {borderEnabled
                  ? formatPhysicalMeasurement(
                      borderEditor.widthUm,
                      presentationUnit,
                    )
                  : "sem borda"}
              </output>
            </span>
            <input
              aria-label="Espessura da Borda"
              className="ui-range"
              max={Math.max(5_000, borderEditor.widthUm)}
              min="0"
              step="250"
              type="range"
              value={borderEnabled ? borderEditor.widthUm : 0}
              onChange={(event) =>
                changeBorderWidth(Number(event.currentTarget.value))
              }
            />
          </label>
        </div>
        {/* PLACEHOLDER UI: frame gap awaits its persistence contract. */}
        <label
          className="ui-range-control"
          data-placeholder-feature="album-design-frame-gap"
        >
          <span className="ui-range-control__heading">
            <span>Espaço entre Frames</span>
            <output>
              {formatPhysicalMeasurement(frameGapUm, presentationUnit)}
            </output>
          </span>
          <input
            aria-label="Espaço entre Frames"
            className="ui-range"
            max="24000"
            min="0"
            step="1000"
            type="range"
            value={frameGapUm}
            onChange={(event) =>
              setFrameGapUm(Number(event.currentTarget.value))
            }
          />
        </label>
      </section>
    </form>
  );
}

function VisualDefaultControl({
  children,
  decorativeMedia,
  label,
  mediaPreviewUrls,
  noneSelected = false,
  open,
  onClear,
  onOpenChange,
  onSelect,
  selectedMediaId,
}: {
  children?: ReactNode;
  decorativeMedia: readonly MediaCatalogItem[];
  label: "Background" | "Overlay";
  mediaPreviewUrls: Readonly<Record<string, string>>;
  /**
   * Verdadeiro apenas quando o escopo inteiro está sem Overlay. Escopo com
   * lados divergentes não é ausência, e não deve marcar `Sem Overlay`.
   */
  noneSelected?: boolean;
  open: boolean;
  onClear?: () => void;
  onOpenChange(open: boolean): void;
  onSelect(mediaId: string): void;
  selectedMediaId: string | null;
}) {
  return (
    <div className="visual-default-field">
      <span className="visual-default-label">{label}</span>
      <div
        aria-label={`Opções de ${label}`}
        className="visual-default-picker"
        role="group"
      >
        {children}
        {onClear ? (
          <button
            aria-label="Sem Overlay"
            aria-pressed={noneSelected}
            className="visual-default-picker__option"
            title="Sem Overlay"
            type="button"
            onClick={onClear}
          >
            <span
              aria-hidden="true"
              className="visual-default-picker__tile visual-default-picker__preview--none"
            />
          </button>
        ) : null}
        <span aria-hidden="true" className="visual-default-picker__divider" />
        <DecorativeMediaPicker
          decorativeMedia={decorativeMedia}
          label={label}
          mediaPreviewUrls={mediaPreviewUrls}
          open={open}
          selectedMediaId={selectedMediaId}
          onOpenChange={onOpenChange}
          onSelect={onSelect}
        />
      </div>
    </div>
  );
}

function albumDesignPreviewDraft(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
  mediaPreviewUrls: Readonly<Record<string, string>>,
): VisualPersonalizationPreview {
  const previewUrl = (mediaId: string) => mediaPreviewUrls[mediaId] ?? "";
  const backgroundContent = (content: ProjectedBackgroundContent) =>
    content.kind === "color"
      ? content
      : { kind: "image" as const, previewUrl: previewUrl(content.mediaId) };
  const overlayContent = (content: ProjectedOverlayContent | null) =>
    content
      ? { kind: "image" as const, previewUrl: previewUrl(content.mediaId) }
      : null;

  return {
    fixedScope: scope,
    background: mapScopedValue(defaults.background, backgroundContent),
    overlay: mapScopedValue(defaults.overlay, overlayContent),
    frameBorder: defaults.frameBorder,
  };
}

function backgroundAtScope(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
) {
  const read = readScopedValue(defaults.background, scope, sameBackground);
  return read.kind === "uniform" ? read.value : null;
}

function overlayAtScope(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
) {
  const read = readScopedValue(defaults.overlay, scope, sameOverlay);
  return read.kind === "uniform" ? read.value : undefined;
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

function scopeLabel(scope: AlbumDesignScope) {
  if (scope === "left") return "Página esquerda";
  if (scope === "right") return "Página direita";
  return "Ambos os lados";
}

const ALBUM_DESIGN_SCOPE_PRESENTATION = {
  accessiblePreviewLabel: "Composição do padrão visual do Álbum",
  externalSelection: false,
  scopeControlsLabel: "Escopo do padrão visual do Álbum",
  technicalGuides: false,
} as const;
