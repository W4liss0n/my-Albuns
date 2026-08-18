import type { CSSProperties, ReactNode } from "react";

import { ActionButton } from "./ActionButton";
import type { DialogAction } from "./DialogAction";
import { DialogWindowFrame } from "./DialogWindowFrame";

type DeterminateProgress = {
  completed: number;
  kind: "determinate";
  remaining?: ReactNode;
  status: ReactNode;
  total: number;
};

type IndeterminateProgress = {
  kind: "indeterminate";
  note?: ReactNode;
  status: ReactNode;
};

type BatchProgress = {
  completed: number;
  currentItem: ReactNode;
  currentItemStatus: ReactNode;
  kind: "batch";
  summary?: ReactNode;
  total: number;
};

export type ProgressDialogState =
  | BatchProgress
  | DeterminateProgress
  | IndeterminateProgress;

interface ProgressDialogProps {
  cancelAction?: DialogAction;
  progress: ProgressDialogState;
  title: string;
}

export function ProgressDialog({
  cancelAction,
  progress,
  title,
}: ProgressDialogProps) {
  const measured = progress.kind !== "indeterminate";
  const total = measured ? Math.max(1, progress.total) : undefined;
  const completed = measured
    ? Math.min(Math.max(0, progress.completed), total ?? 1)
    : undefined;
  const percentage = measured
    ? Math.round(((completed ?? 0) / (total ?? 1)) * 100)
    : undefined;
  const indicatorStyle = measured
    ? ({ "--ui-progress-width": `${percentage}%` } as CSSProperties)
    : undefined;

  return (
    <DialogWindowFrame
      actions={
        cancelAction ? (
          <ActionButton
            disabled={cancelAction.disabled}
            onClick={cancelAction.onClick}
          >
            {cancelAction.label}
          </ActionButton>
        ) : undefined
      }
      layout="progress"
      title={title}
    >
      <div className="ui-progress-dialog">
        {progress.kind === "batch" ? (
          <>
            <ProgressBar
              completed={completed}
              indicatorStyle={indicatorStyle}
              title={title}
              total={total}
            />
            <div
              aria-live="polite"
              className="ui-progress-dialog__current-item"
              role="status"
            >
              <span aria-hidden="true" className="ui-progress-dialog__spinner" />
              <span className="ui-progress-dialog__item-name">
                {progress.currentItem}
              </span>
              <span className="ui-progress-dialog__item-status">
                {progress.currentItemStatus}
              </span>
            </div>
            <div className="ui-progress-dialog__meta">
              <span>
                Álbum {Math.min((completed ?? 0) + 1, total ?? 1)} de {total}
              </span>
              {progress.summary ? <span>{progress.summary}</span> : null}
            </div>
          </>
        ) : (
          <>
            <p
              aria-live="polite"
              className="ui-progress-dialog__status"
              role="status"
            >
              {progress.status}
            </p>
            <ProgressBar
              completed={completed}
              indicatorStyle={indicatorStyle}
              title={title}
              total={total}
            />
            {progress.kind === "determinate" ? (
              <div className="ui-progress-dialog__meta">
                <span>{percentage}%</span>
                <span className="ui-progress-dialog__meta-spacer" />
                {progress.remaining ? <span>{progress.remaining}</span> : null}
              </div>
            ) : (
              <p className="ui-progress-dialog__note">
                {progress.note ?? "sem estimativa de tempo"}
              </p>
            )}
          </>
        )}
      </div>
    </DialogWindowFrame>
  );
}

interface ProgressBarProps {
  completed?: number;
  indicatorStyle?: CSSProperties;
  title: string;
  total?: number;
}

function ProgressBar({
  completed,
  indicatorStyle,
  title,
  total,
}: ProgressBarProps) {
  const measured = completed !== undefined && total !== undefined;

  return (
    <div
      aria-label={`Progresso de ${title}`}
      aria-valuemax={measured ? total : undefined}
      aria-valuemin={measured ? 0 : undefined}
      aria-valuenow={measured ? completed : undefined}
      className="ui-progress-dialog__track"
      role="progressbar"
    >
      <span
        aria-hidden="true"
        className={
          measured
            ? "ui-progress-dialog__indicator"
            : "ui-progress-dialog__indicator ui-progress-dialog__indicator--indeterminate"
        }
        style={indicatorStyle}
      />
    </div>
  );
}
