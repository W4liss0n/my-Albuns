import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
} from "react";

import {
  WORKSPACE_PANEL_DEFAULTS,
  WORKSPACE_PANEL_SIZE_LIMITS,
  type WorkspacePanel,
  type WorkspacePanelPreference,
} from "../application/workspacePreferences";
import "./WorkspacePanelLayout.css";

interface WorkspacePanelDefinition {
  className: string;
  controls: string;
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
    dimension: "width",
    increaseKey: "ArrowLeft",
    label: "Redimensionar Painel contextual",
    maximumSize: WORKSPACE_PANEL_SIZE_LIMITS.inspector.maximum,
    minimumSize: WORKSPACE_PANEL_SIZE_LIMITS.inspector.minimum,
    minimumWorkAreaSize: 480,
    orientation: "vertical",
  },
  media: {
    className: "media-splitter",
    controls: "media-panel",
    dimension: "height",
    increaseKey: "ArrowUp",
    label: "Redimensionar Painel de imagens",
    maximumSize: WORKSPACE_PANEL_SIZE_LIMITS.media.maximum,
    minimumSize: WORKSPACE_PANEL_SIZE_LIMITS.media.minimum,
    minimumWorkAreaSize: 240,
    orientation: "horizontal",
  },
};

type WorkspacePanelStates = Record<WorkspacePanel, WorkspacePanelPreference>;

export function useWorkspacePanelLayout({
  preferences,
  onSizeChange,
  onVisibilityChange,
}: {
  preferences: Readonly<
    Record<WorkspacePanel, WorkspacePanelPreference | null>
  >;
  onSizeChange(panel: WorkspacePanel, size: number): void;
  onVisibilityChange(panel: WorkspacePanel, visible: boolean): void;
}) {
  const [panels, setPanels] = useState<WorkspacePanelStates>(() =>
    normalizedPanelStates(preferences),
  );
  const panelsRef = useRef(panels);
  panelsRef.current = panels;
  const workspaceRef = useRef<HTMLDivElement>(null);
  const activePanelRef = useRef<WorkspacePanel | null>(null);

  useEffect(() => {
    const next = normalizedPanelStates(preferences);
    panelsRef.current = next;
    setPanels(next);
  }, [
    preferences.inspector?.size,
    preferences.inspector?.visible,
    preferences.media?.size,
    preferences.media?.visible,
  ]);

  const updatePanel = useCallback(
    (
      panel: WorkspacePanel,
      preference: WorkspacePanelPreference,
      persist: boolean,
    ) => {
      const current = panelsRef.current;
      if (
        current[panel].size === preference.size &&
        current[panel].visible === preference.visible
      ) {
        return;
      }
      const next = { ...current, [panel]: preference };
      panelsRef.current = next;
      setPanels(next);
      if (persist) onSizeChange(panel, preference.size);
    },
    [onSizeChange],
  );

  const setPanelSize = useCallback(
    (
      panel: WorkspacePanel,
      candidate: number,
      bounds?: DOMRect,
    ) => {
      const next = constrainPanelSize(panel, candidate, bounds);
      updatePanel(panel, { ...panelsRef.current[panel], size: next }, false);
    },
    [updatePanel],
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
      const activePanel = activePanelRef.current;
      activePanelRef.current = null;
      if (activePanel) onSizeChange(activePanel, panelsRef.current[activePanel].size);
    };

    window.addEventListener("pointermove", resizeActivePanel);
    window.addEventListener("pointerup", finishResize);
    window.addEventListener("pointercancel", finishResize);
    return () => {
      window.removeEventListener("pointermove", resizeActivePanel);
      window.removeEventListener("pointerup", finishResize);
      window.removeEventListener("pointercancel", finishResize);
    };
  }, [onSizeChange, setPanelSize]);

  const beginResize = useCallback((panel: WorkspacePanel) => {
    activePanelRef.current = panel;
  }, []);

  const resizeBy = useCallback(
    (panel: WorkspacePanel, delta: number) => {
      const bounds = workspaceRef.current?.getBoundingClientRect();
      const current = panelsRef.current[panel];
      updatePanel(
        panel,
        {
          ...current,
          size: constrainPanelSize(panel, current.size + delta, bounds),
        },
        true,
      );
    },
    [updatePanel],
  );

  const setPanelVisibility = useCallback(
    (panel: WorkspacePanel, visible: boolean) => {
      updatePanel(panel, { ...panelsRef.current[panel], visible }, false);
      onVisibilityChange(panel, visible);
    },
    [onVisibilityChange, updatePanel],
  );

  const style = {
    "--inspector-width": panels.inspector.visible
      ? `${panels.inspector.size}px`
      : "0px",
    "--inspector-splitter-size": panels.inspector.visible
      ? `${WORKSPACE_SPLITTER_SIZE}px`
      : "0px",
    "--media-panel-height": panels.media.visible
      ? `${panels.media.size}px`
      : "0px",
    "--media-splitter-size": panels.media.visible
      ? `${WORKSPACE_SPLITTER_SIZE}px`
      : "0px",
  } as CSSProperties;

  return {
    beginResize,
    panels,
    resizeBy,
    setPanelVisibility,
    style,
    workspaceRef,
  };
}

export function WorkspacePanelSplitter({
  disabled = false,
  panel,
  size,
  onResizeStart,
  onResizeBy,
}: {
  disabled?: boolean;
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
      disabled ||
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
      aria-disabled={disabled || undefined}
      aria-orientation={definition.orientation}
      aria-valuemin={definition.minimumSize}
      aria-valuemax={definition.maximumSize}
      aria-valuenow={Math.round(size)}
      tabIndex={disabled ? -1 : 0}
      onPointerDown={(event) => {
        if (disabled || event.button !== 0) return;
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

function normalizedPanelStates(
  preferences: Readonly<
    Record<WorkspacePanel, WorkspacePanelPreference | null>
  >,
): WorkspacePanelStates {
  return {
    inspector: normalizePanelState("inspector", preferences.inspector),
    media: normalizePanelState("media", preferences.media),
  };
}

function normalizePanelState(
  panel: WorkspacePanel,
  preference: WorkspacePanelPreference | null,
) {
  const fallback = WORKSPACE_PANEL_DEFAULTS[panel];
  return {
    size: constrainPanelSize(panel, preference?.size ?? fallback.size),
    visible: preference?.visible ?? fallback.visible,
  };
}
