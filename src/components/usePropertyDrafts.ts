import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection, FrameStyleChange, FrameStyleEdit, PhotoAngleEdit, PhotoZoomEdit, ProjectIntent } from "../domain/project";
import { useFrameCompositionDraft } from "./useFrameCompositionDraft";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";

type PropertyValue =
  | { kind: "frameStyle"; change: FrameStyleChange }
  | { kind: "photoAngle"; angleTenths: number }
  | { kind: "photoZoom"; percent: number }
  | { kind: "singlePhotoZoom"; frameId: string; startValue: number; value: number };

interface PropertyDraftsInput {
  projection: EditorProjection;
  frameIds: readonly string[];
  disabled: boolean;
  port: ProjectCorePort;
  runner: ProjectMutationRunner;
  commitFrameStyle(edit: FrameStyleEdit): Promise<EditorProjection | null>;
  commitPhotoAngle(edit: PhotoAngleEdit): Promise<EditorProjection | null>;
  commitPhotoZoom(edit: PhotoZoomEdit): Promise<EditorProjection | null>;
  commitInteraction(intent: ProjectIntent): Promise<boolean>;
  onError(message: string): void;
}

/** One selected property owns preview, settlement and synchronous queue admission. */
export function usePropertyDrafts(input: PropertyDraftsInput) {
  const frames = input.projection.state.album.sheets.flatMap(sheet => sheet.frames)
    .filter(frame => input.frameIds.includes(frame.id));
  const canEdit = frames.length > 0 && !input.disabled;
  const canEditPhoto = canEdit && frames.some(frame => frame.photo !== null);
  const selected = frames.length === 1 ? frames[0] : null;
  const draft = useFrameCompositionDraft<PropertyValue>({
    ...input, session: input.port, disabled: !canEdit,
    propertyKey: value => value.kind === "frameStyle" ? `${value.kind}:${value.change.kind}` : value.kind,
    resolve: (frameIds, value) => {
      switch (value.kind) {
        case "frameStyle": return input.port.previewFrameStyle({ frameIds, change: value.change });
        case "photoAngle": return input.port.previewPhotoAngle({ frameIds, angleTenths: value.angleTenths });
        case "photoZoom": return input.port.previewPhotoZoom({ frameIds, userZoom: value.percent / 100 });
        // The individual slider retains the Canvas transform preview contract.
        case "singlePhotoZoom": return Promise.resolve([]);
      }
    },
    commit: async (frameIds, value) => {
      switch (value.kind) {
        case "frameStyle": return (await input.commitFrameStyle({ frameIds, change: value.change })) !== null;
        case "photoAngle": return (await input.commitPhotoAngle({ frameIds, angleTenths: value.angleTenths })) !== null;
        case "photoZoom": return (await input.commitPhotoZoom({ frameIds, userZoom: value.percent / 100 })) !== null;
        case "singlePhotoZoom": {
          const deltaZoom = Number((value.value - value.startValue).toFixed(4));
          if (Math.abs(deltaZoom) < 0.0001) return true;
          return input.commitInteraction({ kind: "transformPhoto", frameId: value.frameId,
            deltaPanX: 0, deltaPanY: 0, deltaZoom });
        }
      }
    },
  });
  function beginZoomGesture() {
    if (!canEditPhoto || !selected?.photo) return;
    if (draft.peek()?.kind === "singlePhotoZoom") return;
    const value = selected.photo.transform.userZoom;
    draft.preview({ kind: "singlePhotoZoom", frameId: selected.id, startValue: value, value });
  }
  function updateZoomGesture(value: number) {
    if (!canEditPhoto || !selected?.photo) return;
    const pending = draft.peek();
    draft.preview(pending?.kind === "singlePhotoZoom" ? { ...pending, value }
      : { kind: "singlePhotoZoom", frameId: selected.id, startValue: selected.photo.transform.userZoom, value });
  }
  const pendingZoom = draft.pendingValues.find(value => value.kind === "singlePhotoZoom");
  const singleZoom = draft.value?.kind === "singlePhotoZoom" ? draft.value : pendingZoom ?? null;
  const control = { scopeKey: draft.scopeKey, settlement: draft.settlement, onCancel: draft.cancel };
  return {
    composition: draft.composition,
    flush: () => { void draft.commit(); },
    frameStyle: { ...control, disabled: !canEdit,
      onPreview: (change: FrameStyleChange) => { if (canEdit) draft.preview({ kind: "frameStyle", change }); },
      onCommit: (change?: FrameStyleChange) => { if (canEdit) void draft.commit(change === undefined ? undefined : { kind: "frameStyle", change }); },
    },
    photoAngle: { ...control, disabled: !canEditPhoto,
      onPreview: (angleTenths: number) => { if (canEditPhoto) draft.preview({ kind: "photoAngle", angleTenths }); },
      onCommit: (angleTenths: number) => { if (canEditPhoto) void draft.commit({ kind: "photoAngle", angleTenths }); },
    },
    photoZoom: { ...control, disabled: !canEditPhoto,
      onPreview: (percent: number) => { if (canEditPhoto) draft.preview({ kind: "photoZoom", percent }); },
      onCommit: (percent: number) => { if (canEditPhoto) void draft.commit({ kind: "photoZoom", percent }); },
    },
    singleZoom: { preview: singleZoom ? { frameId: singleZoom.frameId, value: singleZoom.value } : null,
      value: singleZoom?.value, committing: pendingZoom !== undefined,
      begin: beginZoomGesture, update: updateZoomGesture,
      finish: async () => { if (draft.peek()?.kind === "singlePhotoZoom") await draft.commit(); },
    },
  };
}
