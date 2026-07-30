import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
} from "react";

import {
  readWorkspacePanelSize,
  writeWorkspacePanelSize,
} from "../state/workspacePreferences";

export type WorkspacePanel = "inspector" | "media";

interface WorkspacePanelDefinition {
  className: string;
  controls: string;
  defaultSize: number;
  dimension: "width" | "height";
  increaseKey: string;
  label: string;
  maximumSize: number;
  minimumSize: number;
  minimumWorkAreaSize: number;
  orientation: "horizontal" | "vertical";
}

const WORKSPACE_SPLITTER_SIZE = 6;
const KEYBOARD_RESIZE_STEP = 12;

const PANEL_DEFINITIONS: Record<
  WorkspacePanel,
  WorkspacePanelDefinition
> = {
  inspector: {
    className: "inspector-splitter",
    controls: "contextual-panel",
    defaultSize: 286,
    dimension: "width",
    increaseKey: "ArrowLeft",
    label: "Redimensionar Painel contextual",
    maximumSize: 480,
    minimumSize: 220,
    minimumWorkAreaSize: 480,
    orientation: "vertical",
  },
  media: {
    className: "media-splitter",
    controls: "media-panel",
    defaultSize: 190,
    dimension: "height",
    increaseKey: "ArrowUp",
    label: "Redimensionar Painel de imagens",
    maximumSize: 360,
    minimumSize: 120,
    minimumWorkAreaSize: 240,
    orientation: "horizontal",
  },
};

interface WorkspacePanelSizes {
  inspector: number;
  media: number;
}

export function useWorkspacePanelLayout() {
  const [sizes, setSizes] = useState<WorkspacePanelSizes>(() => ({
    inspector: readPanelSize("inspector"),
    media: readPanelSize("media"),
  }));
  const workspaceRef = useRef<HTMLDivElement>(null);
  const activePanelRef = useRef<WorkspacePanel | null>(null);

  const setPanelSize = useCallback(
    (
      panel: WorkspacePanel,
      candidate: number,
      bounds?: DOMRect,
    ) => {
      const next = constrainPanelSize(panel, candidate, bounds);
      setSizes((current) =>
        current[panel] === next
          ? current
          : { ...current, [panel]: next },
      );
      writeWorkspacePanelSize(panel, next);
    },
    [],
  );

  useEffect(() => {
    const resizeActivePanel = (event: PointerEvent) => {
      const workspace = workspaceRef.current;
      const activePanel = activePanelRef.current;
      if (!workspace || !activePanel) return;

      const definition = PANEL_DEFINITIONS[activePanel];
      const bounds = workspace.getBoundingClientRect();
      const pointerPosition =
        definition.dimension === "width"
          ? event.clientX
          : event.clientY;
      const farEdge =
        definition.dimension === "width"
          ? bounds.right
          : bounds.bottom;
      setPanelSize(activePanel, farEdge - pointerPosition, bounds);
    };
    const finishResize = () => {
      activePanelRef.current = null;
    };

    window.addEventListener("pointermove", resizeActivePanel);
    window.addEventListener("pointerup", finishResize);
    window.addEventListener("pointercancel", finishResize);
    return () => {
      window.removeEventListener("pointermove", resizeActivePanel);
      window.removeEventListener("pointerup", finishResize);
      window.removeEventListener("pointercancel", finishResize);
    };
  }, [setPanelSize]);

  const beginResize = useCallback((panel: WorkspacePanel) => {
    activePanelRef.current = panel;
  }, []);

  const resizeBy = useCallback(
    (panel: WorkspacePanel, delta: number) => {
      const bounds = workspaceRef.current?.getBoundingClientRect();
      setSizes((current) => {
        const next = constrainPanelSize(
          panel,
          current[panel] + delta,
          bounds,
        );
        writeWorkspacePanelSize(panel, next);
        return current[panel] === next
          ? current
          : { ...current, [panel]: next };
      });
    },
    [],
  );

  const style = {
    "--inspector-width": `${sizes.inspector}px`,
    "--media-panel-height": `${sizes.media}px`,
  } as CSSProperties;

  return {
    beginResize,
    resizeBy,
    sizes,
    style,
    workspaceRef,
  };
}

export function WorkspacePanelSplitter({
  panel,
  size,
  onResizeStart,
  onResizeBy,
}: {
  panel: WorkspacePanel;
  size: number;
  onResizeStart(panel: WorkspacePanel): void;
  onResizeBy(panel: WorkspacePanel, delta: number): void;
}) {
  const definition = PANEL_DEFINITIONS[panel];
  const decreaseKey =
    definition.orientation === "vertical"
      ? "ArrowRight"
      : "ArrowDown";

  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (
      event.key !== definition.increaseKey &&
      event.key !== decreaseKey
    ) {
      return;
    }

    event.preventDefault();
    onResizeBy(
      panel,
      event.key === definition.increaseKey
        ? KEYBOARD_RESIZE_STEP
        : -KEYBOARD_RESIZE_STEP,
    );
  }

  return (
    <div
      className={`workspace-splitter ${definition.className}`}
      role="separator"
      aria-label={definition.label}
      aria-controls={definition.controls}
      aria-orientation={definition.orientation}
      aria-valuemin={definition.minimumSize}
      aria-valuemax={definition.maximumSize}
      aria-valuenow={Math.round(size)}
      tabIndex={0}
      onPointerDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        onResizeStart(panel);
      }}
      onKeyDown={handleKeyDown}
    />
  );
}

function constrainPanelSize(
  panel: WorkspacePanel,
  candidate: number,
  bounds?: DOMRect,
) {
  const definition = PANEL_DEFINITIONS[panel];
  const workspaceSize =
    definition.dimension === "width"
      ? bounds?.width
      : bounds?.height;
  const availableMaximum =
    workspaceSize && workspaceSize > 0
      ? Math.max(
          definition.minimumSize,
          Math.min(
            definition.maximumSize,
            workspaceSize -
              definition.minimumWorkAreaSize -
              WORKSPACE_SPLITTER_SIZE,
          ),
        )
      : definition.maximumSize;

  return Math.min(
    availableMaximum,
    Math.max(definition.minimumSize, candidate),
  );
}

function readPanelSize(panel: WorkspacePanel) {
  const definition = PANEL_DEFINITIONS[panel];
  return constrainPanelSize(
    panel,
    readWorkspacePanelSize(panel, definition.defaultSize),
  );
}
