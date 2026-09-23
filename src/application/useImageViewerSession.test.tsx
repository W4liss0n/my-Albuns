import { StrictMode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { ImageViewerWindowPort, ViewerCorrectionAction } from "./imageViewerWindow";
import type { ProjectMutationRunner } from "../components/useProjectMutationRunner";
import { representativeProjection } from "../test/projectFixtures";
import { useImageViewerSession } from "./useImageViewerSession";

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
