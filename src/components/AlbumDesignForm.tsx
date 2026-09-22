import { VisualScopePreview } from "./VisualScopePreview";
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useState,
} from "react";

import {
  changeFrameBorderColor,
  changeFrameBorderWidth,
  createFrameBorderEditorState,
} from "../application/frameBorderEditor";
import { ColorPropertyControl } from "../ui/ColorPropertyControl";
import { FrameDefaultRangeControl } from "./FrameDefaultRangeControl";
import { renderableMediaPreviewUrls } from "../application/mediaPreviews";
import {
  mapScopedValue,
  readScopedValue,
} from "../application/scopedValues";
import {
  createAlbumDesignProjectDraft,
  type AlbumDesignProjectDraft,
  type AlbumDesignValue,
} from "../application/projectSettingsDraft";
import type { MediaPreview } from "../application/projectPorts";
import type {
  DisplayUnit,
  DocumentSnapshot,
  MediaCatalogItem,
  ProjectedBackgroundContent,
  ProjectedOverlayContent,
  ProjectedVisualDefaults,
} from "../domain/project";
import {
  ProportionalPreviewViewport,
  type DecorativePreview,
  type VisualPersonalizationPreview,
  type VisualPreviewGeometry,
} from "../ui/visualPreview";
import {
  setAlbumBackground,
  setAlbumFrameBorder,
  setAlbumOverlay,
  type AlbumDesignScope,
} from "./albumDesignDraft";
import { VisualDesignControl } from "./VisualDesignControl";
import { useSemanticBaseline } from "./useSemanticBaseline";
import "./AlbumDesignForm.css";

const DEFAULT_FRAME_BORDER = { rgb: "#2C2924", widthUm: 1_000 };

interface AlbumDesignFormProps {
  document: DocumentSnapshot;
  presentationUnit: DisplayUnit;
  formId: string;
  mediaItems: readonly MediaCatalogItem[];
  mediaPreviews: Readonly<Record<string, MediaPreview>>;
  revision: number;
  value: ProjectedVisualDefaults;
  frameGapUm: number;
  onApply(draft: AlbumDesignProjectDraft): Promise<boolean>;
  onReadyChange(ready: boolean): void;
}

interface AlbumDesignDraftSession {
  current: AlbumDesignProjectDraft;
  pending: {
    attempt: AlbumDesignProjectDraft;
    submitted: AlbumDesignProjectDraft;
    subsequent: AlbumDesignProjectDraft;
    settled: boolean;
  } | null;
}

export function AlbumDesignForm({
  document,
  presentationUnit,
  formId,
  mediaItems,
  mediaPreviews,
  revision,
  value,
  frameGapUm: confirmedFrameGapUm,
  onApply,
  onReadyChange,
}: AlbumDesignFormProps) {
  const baselineValue = { ...value, frameGapUm: confirmedFrameGapUm };
  const baselineSignature = JSON.stringify(baselineValue);
  const baseline = useSemanticBaseline(
    { revision, value: baselineValue },
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
  const applySettled = draftSession.pending?.settled ?? false;
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
  const mediaPreviewUrls = useMemo(
    () => renderableMediaPreviewUrls(mediaPreviews),
    [mediaPreviews],
  );
  const frameGapUm = draft.frameGapUm;
  /**
   * Um seletor de Decorativo por vez. O estado vive aqui, e não em cada
   * controle, para que abrir um feche o outro por construção — inclusive
   * quando a abertura vem do teclado, que não emite `pointerdown`.
   */
  const [openPicker, setOpenPicker] = useState<
    "Fundo" | "Sobreposição" | null
  >(null);
  const [applying, setApplying] = useState(false);
  const dirty = projectDraft.changed;
  const ready = dirty && !applying;
  const background = readScopedValue(draft.background, scope, sameBackground);
  const overlay = readScopedValue(draft.overlay, scope, sameOverlay);
  const borderEnabled = draft.frameBorder.kind === "solid";
  const previewPersonalization = albumDesignPreviewDraft(
    draft,
    scope,
    mediaPreviews,
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
      if (!pending) {
        return {
          ...session,
          current: session.current.rebase(
            baseline.revision,
            baseline.value,
          ),
        };
      }
      const submitted = pending.submitted.rebase(
        baseline.revision,
        baseline.value,
      );
      if (pending.settled && !submitted.changed) {
        return {
          current: pending.subsequent.rebase(
            baseline.revision,
            baseline.value,
          ),
          pending: null,
        };
      }
      const subsequent = pending.subsequent.rebase(
        baseline.revision,
        submitted.value,
      );
      return {
        current: submitted.transition(subsequent.value),
        pending: { ...pending, submitted, subsequent },
      };
    });
  }, [applySettled, baseline]);

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
    transition: (current: AlbumDesignValue) => ProjectedVisualDefaults | AlbumDesignValue,
  ) {
    setDraftSession((session) => ({
      current: session.current.transition(
        { ...session.current.value, ...transition(session.current.value) },
      ),
      pending: session.pending
        ? {
            ...session.pending,
            subsequent: session.pending.subsequent.transition(
              { ...session.pending.subsequent.value, ...transition(session.pending.subsequent.value) },
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
        attempt: submitted,
        submitted,
        subsequent: createAlbumDesignProjectDraft(
          submitted.baselineRevision,
          submitted.value,
        ),
        settled: false,
      },
    }));
    setApplying(true);
    let completed = false;
    try {
      completed = await onApply(submitted);
    } finally {
      setDraftSession((session) => {
        const pending = session.pending;
        if (!pending || pending.attempt !== submitted) return session;
        return completed
          ? { ...session, pending: { ...pending, settled: true } }
          : { current: session.current, pending: null };
      });
      setApplying(false);
    }
  }

  return (
    <form
      id={formId}
      className="inspector-subsections album-design-form"
      noValidate
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
            label="Prévia do padrão visual do álbum"
            width={previewGeometry.widthUm}
          >
            <VisualScopePreview
              content={{
                kind: "general",
                label: "Composição do padrão visual do álbum",
                geometry: previewGeometry,
                personalization: previewPersonalization,
                frameGapUm,
              }}
              label="Escopo do padrão visual do álbum"
              scope={scope}
              onScopeChange={setScope}
            />
          </ProportionalPreviewViewport>
        </div>
        <p className="ui-section-eyebrow album-design-scope-label">
          {scopeLabel(scope)}
        </p>
        <VisualDesignControl
          decorativeMedia={decorativeMedia}
          label="Fundo"
          mediaPreviewUrls={mediaPreviewUrls}
          open={openPicker === "Fundo"}
          key={`background:${scope}`}
          values={background.kind === "uniform" ? [background.value] : [background.left, background.right]}
          color={{ label: "do fundo", onCommit: (rgb) => chooseBackground({ kind: "color", rgb }) }}
          onOpenChange={(open) => setOpenPicker(open ? "Fundo" : null)}
          onSelect={(mediaId) => chooseBackground({ kind: "media", mediaId })}
        />
        <VisualDesignControl
          decorativeMedia={decorativeMedia}
          label="Sobreposição"
          mediaPreviewUrls={mediaPreviewUrls}
          open={openPicker === "Sobreposição"}
          values={(overlay.kind === "uniform" ? [overlay.value] : [overlay.left, overlay.right]).map(value => value ?? { kind: "none" })}
          onOpenChange={(open) => setOpenPicker(open ? "Sobreposição" : null)}
          onSelect={(mediaId) => chooseOverlay({ kind: "media", mediaId })}
          onClear={() => chooseOverlay(null)}
        />
      </section>
      <section className="inspector-subsection">
        <h3>Padrão dos quadros</h3>
        <div className="album-frame-border-row">
          <ColorPropertyControl label="da borda" rgb={borderEditor.rgb}
            onCommit={(rgb) => updateBorder(changeFrameBorderColor(
              createFrameBorderEditorState(draft.frameBorder, borderEditor), rgb,
            ))} />
          <FrameDefaultRangeControl
            kind="border"
            displayUnit={presentationUnit}
            includeValueUm={borderEditor.widthUm}
            valueUm={borderEnabled ? borderEditor.widthUm : 0}
            onChange={changeBorderWidth}
          />
        </div>
        <FrameDefaultRangeControl
          kind="gap"
          displayUnit={presentationUnit}
          includeValueUm={frameGapUm}
          valueUm={frameGapUm}
          onChange={(frameGapUm) =>
            transitionProjectDraft((current) => ({ ...current, frameGapUm }))
          }
        />
      </section>
    </form>
  );
}

function albumDesignPreviewDraft(
  defaults: ProjectedVisualDefaults,
  scope: AlbumDesignScope,
  mediaPreviews: Readonly<Record<string, MediaPreview>>,
): VisualPersonalizationPreview {
  const backgroundContent = (content: ProjectedBackgroundContent) =>
    content.kind === "color"
      ? content
      : {
          kind: "image" as const,
          preview: decorativePreview(content.mediaId, mediaPreviews),
        };
  const overlayContent = (content: ProjectedOverlayContent | null) =>
    content
      ? {
          kind: "image" as const,
          preview: decorativePreview(content.mediaId, mediaPreviews),
        }
      : null;

  return {
    fixedScope: scope,
    background: mapScopedValue(defaults.background, backgroundContent),
    overlay: mapScopedValue(defaults.overlay, overlayContent),
    frameBorder: defaults.frameBorder,
  };
}

function decorativePreview(
  mediaId: string,
  mediaPreviews: Readonly<Record<string, MediaPreview>>,
): DecorativePreview {
  const preview = mediaPreviews[mediaId];
  if (!preview) return { state: "pending" as const };
  if (preview.state === "ready") {
    return { state: "ready" as const, url: preview.url };
  }
  if (preview.state === "absent") {
    return { state: "absent" as const };
  }
  return {
    state: "unavailable" as const,
    url: preview.url,
  };
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

function scopeLabel(scope: AlbumDesignScope) {
  if (scope === "left") return "Página esquerda";
  if (scope === "right") return "Página direita";
  return "Ambos os lados";
}
