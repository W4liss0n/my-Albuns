import { expect, test, vi } from "vitest";
import type { ViewerPresentation } from "../application/imageViewerWindow";
import { observeViewerPresentation } from "./viewerObservation";

const presentation = (revision: number): ViewerPresentation => ({
  sessionId: "one", revision, mediaId: `image-${revision}`, name: "Imagem",
  url: null, state: "loading", canPrevious: false, canNext: false,
});

test("viewer keeps a live revision delivered while its initial snapshot is pending", async () => {
  let emit!: (value: ViewerPresentation) => void;
  let resolve!: (value: ViewerPresentation | null) => void;
  const stop = vi.fn();
  const receive = vi.fn();
  const observation = observeViewerPresentation({
    onPresentation: async callback => { emit = callback; return stop; },
    current: () => new Promise(value => { resolve = value; }),
  }, receive);
  await Promise.resolve();
  emit(presentation(3));
  resolve(presentation(1));
  await observation.ready;
  emit(presentation(2));
  expect(receive).toHaveBeenCalledTimes(1);
  expect(receive).toHaveBeenLastCalledWith(presentation(3));
  observation.dispose();
  expect(stop).toHaveBeenCalledOnce();
});

test("viewer subscription failure is reported and does not read or deliver a snapshot", async () => {
  const read = vi.fn(async () => presentation(1));
  const receive = vi.fn();
  const observation = observeViewerPresentation({
    onPresentation: async () => { throw new Error("listener unavailable"); }, current: read,
  }, receive);
  await expect(observation.ready).rejects.toThrow("listener unavailable");
  expect(read).not.toHaveBeenCalled();
  expect(receive).not.toHaveBeenCalled();
  observation.dispose();
});
