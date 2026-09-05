import { act, renderHook } from "@testing-library/react";
import { expect, test } from "vitest";
import type { ImageProcessingProgress } from "../application/projectPorts";
import { useImageProcessing } from "./useImageProcessing";

test("replacing an idle operation context does not trigger another state update", () => {
  let renders = 0;
  const view = renderHook(({ operationContext }) => {
    renders += 1;
    return useImageProcessing("project", operationContext);
  }, { initialProps: { operationContext: {} } });
  const previousRenders = renders;
  view.rerender({ operationContext: {} });
  expect(renders).toBe(previousRenders + 1);
  expect(view.result.current.progress).toBeNull();
  expect(view.result.current.problems).toEqual([]);
});

test("a retired Project cannot overwrite progress or warnings of its replacement", async () => {
  const operationContext = {};
  const view = renderHook(({ projectId }) => useImageProcessing(projectId, operationContext), {
    initialProps: { projectId: "old" },
  });
  let publishOld!: (progress: ImageProcessingProgress) => void;
  let finishOld!: () => void;
  let old!: Promise<void>;
  act(() => {
    old = view.result.current.run((publish) => {
      publishOld = publish;
      publish({ completedFiles: 0, totalFiles: 5 });
      return new Promise<void>((resolve) => { finishOld = resolve; });
    });
  });
  view.rerender({ projectId: "new" });
  expect(view.result.current.progress).toBeNull();
  let finishNew!: () => void;
  let current!: Promise<void>;
  act(() => {
    current = view.result.current.run((publish) => {
      publish({ completedFiles: 0, totalFiles: 2 });
      return new Promise<void>((resolve) => { finishNew = resolve; });
    });
  });
  await act(async () => {
    publishOld({ completedFiles: 5, totalFiles: 5, problem: { fileName: "Antiga.jpg", reason: "Cache antigo" } });
    finishOld();
    await old;
  });
  expect(view.result.current.progress).toEqual({ completedFiles: 0, totalFiles: 2 });
  expect(view.result.current.problems).toEqual([]);
  await act(async () => { finishNew(); await current; });
  expect(view.result.current.progress).toBeNull();
});

test("keeps problems from completed image actions until they are dismissed", async () => {
  const operationContext = {};
  const view = renderHook(() => useImageProcessing("project", operationContext));
  const problem = { fileName: "Foto.jpg", reason: "Cache indisponível" };
  await act(async () => {
    await view.result.current.run(async (publish) => {
      publish({ completedFiles: 1, totalFiles: 1, problem });
    });
    await view.result.current.run(async (publish) => {
      publish({ completedFiles: 1, totalFiles: 1 });
    });
  });
  expect(view.result.current.problems).toEqual([problem]);
  act(() => view.result.current.dismissProblems());
  expect(view.result.current.problems).toEqual([]);
});
