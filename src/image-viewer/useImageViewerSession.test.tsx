import { StrictMode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { ImageViewerWindowPort, ViewerCorrectionAction, ViewerPresentation } from "../application/imageViewerWindow";
import type { ProjectMutationRunner } from "../application/projectMutation";
import { representativeProjection } from "../test/projectFixtures";
import { useImageViewerSession } from "../application/useImageViewerSession";

function origin() {
  const button = document.createElement("button");
  document.body.append(button);
  button.focus();
  return button;
}

function session(port: ImageViewerWindowPort) {
  return renderHook(() => useImageViewerSession({
    projectId: representativeProjection.state.projectId,
    media: representativeProjection.state.album.media,
    previews: {}, previewUrls: {}, port,
    mutation: { run: vi.fn(), waitForIdle: vi.fn() } as unknown as ProjectMutationRunner,
    onProjectionChange: vi.fn(),
  }), { wrapper: StrictMode });
}

test("strict mode closes a correcting session with one cancellation and one focus restoration", async () => {
  let closed!: (sessionId: string) => void;
  let correction!: (action: ViewerCorrectionAction) => void;
  const cancelCorrection = vi.fn(async () => undefined);
  const port: ImageViewerWindowPort = {
    open: vi.fn(async () => undefined), update: vi.fn(async () => undefined), close: vi.fn(async () => undefined),
    onNavigate: vi.fn(async () => () => undefined),
    onClosed: vi.fn(async (callback) => { closed = callback; return () => undefined; }),
    onCorrection: vi.fn(async (callback) => { correction = callback; return () => undefined; }),
    cancelCorrection,
  };
  const button = origin();
  const focus = vi.spyOn(button, "focus");
  const view = session(port);
  act(() => view.result.current.open("panel", "media-001", ["media-001"], button));
  await waitFor(() => expect(port.open).toHaveBeenCalledOnce());
  const sessionId = vi.mocked(port.open).mock.calls[0][0].sessionId;
  act(() => correction({ sessionId, kind: "start" }));
  await waitFor(() => expect(view.result.current.active?.correction?.phase).toBe("browse"));
  act(() => { closed(sessionId); closed(sessionId); });
  await waitFor(() => expect(view.result.current.active).toBeNull());
  await waitFor(() => expect(focus).toHaveBeenCalledOnce());
  expect(cancelCorrection).toHaveBeenCalledOnce();
  view.unmount();
  button.remove();
});

test("a saved correction stays on screen until the Cache publishes the replaced photo", async () => {
  let correction!: (action: ViewerCorrectionAction) => void;
  let latest!: ViewerPresentation;
  const port: ImageViewerWindowPort = {
    open: vi.fn(async (presentation) => { latest = presentation; }),
    update: vi.fn(async (presentation) => { latest = presentation; }),
    close: vi.fn(async () => undefined),
    onNavigate: vi.fn(async () => () => undefined), onClosed: vi.fn(async () => () => undefined),
    onCorrection: vi.fn(async (callback) => { correction = callback; return () => undefined; }),
    prepareCorrection: vi.fn(async () => ({ token: "token", url: "corrected" })),
    cancelCorrection: vi.fn(async () => undefined),
    applyCorrection: vi.fn(async () => representativeProjection),
  };
  const mutation: ProjectMutationRunner = {
    run: async (operation) => ({ status: "completed", projection: await operation({} as never, null) }),
    waitForIdle: async () => null,
  };
  const preview = (mediaId: string, url: string) => ({ mediaId, state: "ready" as const, url });
  const view = renderHook(({ previewUrls }: { previewUrls: Record<string, string> }) => useImageViewerSession({
    projectId: representativeProjection.state.projectId, media: representativeProjection.state.album.media,
    previews: Object.fromEntries(Object.entries(previewUrls).map(([id, url]) => [id, preview(id, url)])), previewUrls, port, mutation,
    onProjectionChange: vi.fn(),
  }), { initialProps: { previewUrls: { "media-001": "original", "media-002": "reference" } } });
  act(() => view.result.current.open("panel", "media-001", ["media-001", "media-003"], null));
  await waitFor(() => expect(port.open).toHaveBeenCalledOnce());
  const send = (kind: ViewerCorrectionAction["kind"]) => act(() => correction({ sessionId: latest.sessionId, kind,
    referenceMediaId: latest.correction?.referenceMediaId, targetUrl: latest.url ?? undefined, referenceUrl: latest.correction?.referenceUrl ?? undefined,
    targetFace: [{ x: .5, y: .5, z: 0 }], referenceFace: [{ x: .5, y: .5, z: 0 }] }));
  send("start");
  await waitFor(() => expect(latest.correction).toMatchObject({ referenceMediaId: "media-002" }));
  send("select");
  send("preview");
  await waitFor(() => expect(latest.correction).toMatchObject({ phase: "preview", resultUrl: "corrected" }));
  send("apply");
  await waitFor(() => expect(latest.correction).toBeUndefined());
  expect(port.applyCorrection).toHaveBeenCalledWith(latest.sessionId, "token");
  expect(latest).toMatchObject({ mediaId: "media-001", url: "corrected", state: "ready" });

  view.rerender({ previewUrls: { "media-001": "replaced", "media-002": "reference" } });
  await waitFor(() => expect(latest.url).toBe("replaced"));
});

test("strict mode restores focus once when opening the native viewer fails", async () => {
  const port: ImageViewerWindowPort = {
    open: vi.fn(async () => { throw new Error("window failed"); }),
    update: vi.fn(async () => undefined), close: vi.fn(async () => undefined),
    onNavigate: vi.fn(async () => () => undefined), onClosed: vi.fn(async () => () => undefined),
  };
  const button = origin();
  const focus = vi.spyOn(button, "focus");
  const view = session(port);
  act(() => view.result.current.open("panel", "media-001", ["media-001"], button));
  await waitFor(() => expect(view.result.current.active).toBeNull());
  await waitFor(() => expect(focus).toHaveBeenCalledOnce());
  view.unmount();
  button.remove();
});
