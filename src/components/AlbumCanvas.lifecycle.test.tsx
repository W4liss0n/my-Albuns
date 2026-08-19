import { StrictMode } from "react";
import { act, render, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { LogEvent, Logger } from "../application/logging";
import { composition } from "./albumCanvasTestFixtures";
import type { CanvasGraphicsDiagnosticProbe } from "./canvasGraphicsDiagnosticProbeContext";
import { createContinuousCanvasLayout } from "./canvasGeometry";
import { LoggingProvider } from "./loggingContext";
import {
  AlbumCanvas,
  finishPixiInitialization,
  getPixiLifecycle,
  renderCanvas,
  setupAlbumCanvasTestHarness,
} from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();
const pixiLifecycle = getPixiLifecycle();

test("waits for PixiJS initialization before destroying an abandoned Canvas", async () => {
  const view = renderCanvas();

  expect(pixiLifecycle.instances).toHaveLength(1);
  expect(() => view.unmount()).not.toThrow();

  await finishPixiInitialization();

  expect(pixiLifecycle.instances[0].destroyCount).toBe(1);
});

test("reports when the actual Pixi Canvas diagnostic rejects WebGL2", async () => {
  const onGraphicsUnavailable = vi.fn();
  const canvasGraphicsDiagnosticProbe =
    vi.fn<CanvasGraphicsDiagnosticProbe>(() => ({
      supported: false,
      code: "webgl2_unavailable",
      renderer: "indisponível",
      reason: "O Canvas real não disponibilizou WebGL2.",
      limits: null,
    }));
  const view = renderCanvas({
    onGraphicsUnavailable,
    canvasGraphicsDiagnosticProbe,
  });

  await finishPixiInitialization();

  await waitFor(() => {
    expect(onGraphicsUnavailable).toHaveBeenCalledWith(
      expect.objectContaining({
        supported: false,
        code: "webgl2_unavailable",
      }),
    );
  });
  expect(await view.findByRole("alert")).toHaveTextContent(
    "O editor gráfico está indisponível.",
  );
  expect(canvasGraphicsDiagnosticProbe).toHaveBeenCalledWith(
    pixiLifecycle.instances[0].canvas,
  );
});

test("blocks the Canvas while WebGL2 is lost and resumes the same Canvas after restoration", async () => {
  const logEvents: LogEvent[] = [];
  const view = renderCanvas({
    logger: {
      write: (event) => logEvents.push(event),
    },
  });
  await finishPixiInitialization();
  const canvas = view.getByLabelText(
    /Canvas contínuo do Álbum/,
  ) as HTMLCanvasElement;
  const contextLost = new Event("webglcontextlost", {
    cancelable: true,
  });

  act(() => {
    canvas.dispatchEvent(contextLost);
  });

  expect(contextLost.defaultPrevented).toBe(true);
  expect(view.getByRole("status")).toHaveTextContent(
    "Restaurando o contexto gráfico",
  );
  expect(view.onTransformPreview).toHaveBeenCalledWith(null);
  expect(logEvents).toEqual(
    expect.arrayContaining([
      expect.objectContaining({
        component: "canvas",
        event: "canvas_context_lost",
      }),
    ]),
  );

  act(() => {
    canvas.dispatchEvent(new Event("webglcontextrestored"));
  });

  await waitFor(() =>
    expect(view.queryByRole("status")).not.toBeInTheDocument(),
  );
  expect(
    view.getByLabelText(/Canvas contínuo do Álbum/),
  ).toBe(canvas);
  expect(logEvents).toEqual(
    expect.arrayContaining([
      expect.objectContaining({
        component: "canvas",
        event: "canvas_context_restored",
      }),
    ]),
  );
});

test("reports a fatal diagnostic when a lost context is not restored", async () => {
  vi.useFakeTimers();
  const onGraphicsUnavailable = vi.fn();
  const view = renderCanvas({ onGraphicsUnavailable });
  await finishPixiInitialization();
  const canvas = view.getByLabelText(/Canvas contínuo do Álbum/);

  act(() => {
    canvas.dispatchEvent(
      new Event("webglcontextlost", { cancelable: true }),
    );
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(10_000);
  });

  expect(onGraphicsUnavailable).toHaveBeenCalledWith(
    expect.objectContaining({
      supported: false,
      code: "context_restore_failed",
    }),
  );
});

test("does not let an abandoned StrictMode initialization destroy the active Canvas", async () => {
  const logEvents: LogEvent[] = [];
  const logger: Logger = {
    write: (event) => logEvents.push(event),
  };
  const view = render(
    <StrictMode>
      <LoggingProvider logger={logger}>
        <AlbumCanvas
          projectId="project-spike-001"
          mode={{ kind: "normal" }}
          composition={composition}
          sheetBarMetadata={[]}
          continuousCanvasLayout={createContinuousCanvasLayout(
            composition.sheets,
          )}
          selectedFrameId={null}
          focusedSheetId="sheet-001"
          centeredSheetId="sheet-001"
          viewport={{ offsetX: 42 }}
          onSelectFrame={() => undefined}
          onFocusSheet={() => undefined}
          onCenteredSheetChange={() => undefined}
          onViewportChange={() => undefined}
          onTransformPreview={() => undefined}
          onTransformCommit={async () => true}
        />
      </LoggingProvider>
    </StrictMode>,
  );

  expect(pixiLifecycle.instances).toHaveLength(2);

  await act(async () => {
    pixiLifecycle.resolveInitializations[1]?.();
    await Promise.resolve();
    pixiLifecycle.resolveInitializations[0]?.();
    await Promise.resolve();
    await Promise.resolve();
  });

  expect(pixiLifecycle.instances[0].destroyCount).toBe(1);
  expect(pixiLifecycle.instances[1].destroyCount).toBe(0);
  expect(pixiLifecycle.instances[1].stage.children).toHaveLength(1);
  const activeWorld = pixiLifecycle.instances[1].stage
    .children[0] as { children: unknown[] };
  expect(activeWorld.children).toHaveLength(1);
  expect(view.container.querySelectorAll("canvas")).toHaveLength(1);

  const initializationStarts = logEvents.filter(
    ({ event }) => event === "canvas_initialization_started",
  );
  const initializationCompleted = logEvents.find(
    ({ event }) => event === "canvas_initialization_completed",
  );
  const initializationAbandoned = logEvents.find(
    ({ event }) => event === "canvas_initialization_abandoned",
  );
  const sceneMaterialized = logEvents.find(
    ({ event }) => event === "canvas_scene_materialized",
  );

  expect(initializationStarts).toHaveLength(2);
  expect(initializationStarts[0].instanceId).not.toBe(
    initializationStarts[1].instanceId,
  );
  expect(initializationCompleted?.instanceId).toBe(
    initializationStarts[1].instanceId,
  );
  expect(initializationAbandoned?.instanceId).toBe(
    initializationStarts[0].instanceId,
  );
  expect(sceneMaterialized?.instanceId).toBe(
    initializationCompleted?.instanceId,
  );
  expect(sceneMaterialized?.projectId).toBe("project-spike-001");
});
