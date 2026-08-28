import { planSheetReorder } from "../application/sheetStructure";
import type { SheetSnapshot } from "../domain/project";

export type SheetReorderSurface = "bar" | "grid";

export type SheetReorderStatus =
  | "idle"
  | "preview"
  | "invalid"
  | "cancelled"
  | "committing";

export interface SheetReorderGhost {
  readonly sheetId: string;
}

export interface SheetReorderSession {
  readonly status: SheetReorderStatus;
  readonly confirmedOrder: readonly string[];
  readonly origin: SheetReorderSurface | null;
  readonly draggedSheetId: string | null;
  readonly targetIndex: number | null;
  readonly previewOrder: readonly string[] | null;
  readonly placeholderIndex: number | null;
  readonly ghost: SheetReorderGhost | null;
}

export type SheetReorderEvent =
  | {
      readonly type: "preview";
      readonly origin: SheetReorderSurface;
      readonly draggedSheetId: string;
      readonly targetIndex: number;
    }
  | {
      readonly type: "drop";
      readonly surface: SheetReorderSurface;
    }
  | { readonly type: "escape" };

export interface SheetReorderCommitEffect {
  readonly sheetId: string;
  readonly targetIndex: number;
}

export interface SheetReorderTransition {
  readonly session: SheetReorderSession;
  readonly effect: SheetReorderCommitEffect | null;
}

export interface SheetReorderRepresentation {
  readonly order: readonly string[];
  readonly placeholderIndex: number | null;
  readonly ghost: SheetReorderGhost | null;
}

export const SHEET_REORDER_AUTO_SCROLL = {
  edgeMarginPx: 72,
  maxVelocityPxPerSecond: {
    horizontal: 960,
    vertical: 720,
  },
} as const;

export const SHEET_REORDER_INVALID_MESSAGE =
  "Posição inválida: Páginas únicas permanecem nas extremidades.";

export interface SheetReorderAutoScrollInput {
  readonly axis: "horizontal" | "vertical";
  readonly pointerPosition: number;
  readonly viewportStart: number;
  readonly viewportEnd: number;
}

export function createSheetReorderSession(
  sheets: readonly SheetSnapshot[],
): SheetReorderSession {
  return {
    status: "idle",
    confirmedOrder: confirmedSheetOrder(sheets),
    origin: null,
    draggedSheetId: null,
    targetIndex: null,
    previewOrder: null,
    placeholderIndex: null,
    ghost: null,
  };
}

export function reduceSheetReorderSession(
  session: SheetReorderSession,
  sheets: readonly SheetSnapshot[],
  event: SheetReorderEvent,
): SheetReorderTransition {
  if (session.status === "committing") {
    return unchanged(session);
  }

  if (event.type === "preview") {
    const plan = planSheetReorder(
      sheets,
      event.draggedSheetId,
      event.targetIndex,
    );
    const valid = plan.valid && plan.changed;

    return {
      effect: null,
      session: {
        status: valid ? "preview" : "invalid",
        confirmedOrder: confirmedSheetOrder(sheets),
        origin: event.origin,
        draggedSheetId: event.draggedSheetId,
        targetIndex: event.targetIndex,
        previewOrder: valid ? plan.order : null,
        placeholderIndex: valid ? plan.targetIndex : null,
        ghost: { sheetId: event.draggedSheetId },
      },
    };
  }

  if (event.type === "escape") {
    if (session.status !== "preview" && session.status !== "invalid") {
      return unchanged(session);
    }
    return cancelled(session.confirmedOrder);
  }

  if (session.status !== "preview" && session.status !== "invalid") {
    return unchanged(session);
  }
  if (event.surface !== session.origin) {
    return cancelled(session.confirmedOrder);
  }
  if (session.status === "invalid") {
    return cancelled(session.confirmedOrder);
  }
  if (
    session.draggedSheetId === null ||
    session.targetIndex === null
  ) {
    return unchanged(session);
  }

  const dropPlan = planSheetReorder(
    sheets,
    session.draggedSheetId,
    session.targetIndex,
  );
  if (!dropPlan.valid || !dropPlan.changed) {
    return cancelled(confirmedSheetOrder(sheets));
  }

  return {
    effect: {
      sheetId: session.draggedSheetId,
      targetIndex: session.targetIndex,
    },
    session: {
      ...session,
      status: "committing",
      confirmedOrder: confirmedSheetOrder(sheets),
      previewOrder: null,
      placeholderIndex: null,
      ghost: null,
    },
  };
}

export function sheetReorderRepresentation(
  session: SheetReorderSession,
  surface: SheetReorderSurface,
): SheetReorderRepresentation {
  if (surface !== session.origin) {
    return confirmedRepresentation(session.confirmedOrder);
  }
  if (session.status === "preview" && session.previewOrder !== null) {
    return {
      order: session.previewOrder,
      placeholderIndex: session.placeholderIndex,
      ghost: session.ghost,
    };
  }
  if (session.status === "invalid") {
    return {
      order: session.confirmedOrder,
      placeholderIndex: null,
      ghost: session.ghost,
    };
  }
  return confirmedRepresentation(session.confirmedOrder);
}

export function sheetReorderAutoScrollVelocity({
  axis,
  pointerPosition,
  viewportStart,
  viewportEnd,
}: SheetReorderAutoScrollInput): number {
  if (
    !Number.isFinite(pointerPosition) ||
    !Number.isFinite(viewportStart) ||
    !Number.isFinite(viewportEnd) ||
    viewportEnd <= viewportStart
  ) {
    return 0;
  }

  const viewportLength = viewportEnd - viewportStart;
  const edgeMargin = Math.min(
    SHEET_REORDER_AUTO_SCROLL.edgeMarginPx,
    viewportLength / 2,
  );
  if (edgeMargin <= 0) {
    return 0;
  }

  const maximumVelocity =
    SHEET_REORDER_AUTO_SCROLL.maxVelocityPxPerSecond[axis];
  const leadingThreshold = viewportStart + edgeMargin;
  if (pointerPosition < leadingThreshold) {
    const proximity = clampUnit(
      (leadingThreshold - pointerPosition) / edgeMargin,
    );
    return -maximumVelocity * proximity * proximity;
  }

  const trailingThreshold = viewportEnd - edgeMargin;
  if (pointerPosition > trailingThreshold) {
    const proximity = clampUnit(
      (pointerPosition - trailingThreshold) / edgeMargin,
    );
    return maximumVelocity * proximity * proximity;
  }

  return 0;
}

function confirmedSheetOrder(
  sheets: readonly SheetSnapshot[],
): readonly string[] {
  return sheets.map((sheet) => sheet.id);
}

function cancelled(
  confirmedOrder: readonly string[],
): SheetReorderTransition {
  return {
    effect: null,
    session: {
      status: "cancelled",
      confirmedOrder,
      origin: null,
      draggedSheetId: null,
      targetIndex: null,
      previewOrder: null,
      placeholderIndex: null,
      ghost: null,
    },
  };
}

function confirmedRepresentation(
  order: readonly string[],
): SheetReorderRepresentation {
  return {
    order,
    placeholderIndex: null,
    ghost: null,
  };
}

function unchanged(session: SheetReorderSession): SheetReorderTransition {
  return { session, effect: null };
}

function clampUnit(value: number): number {
  return Math.min(1, Math.max(0, value));
}
