import { useId, type ReactNode, type Ref } from "react";
import { CircleAlert, CircleHelp } from "lucide-react";

import { ActionButton } from "./ActionButton";
import { AppIcon } from "./AppIcon";
import type { DialogAction } from "./DialogAction";
import { DialogWindowFrame } from "./DialogWindowFrame";
import "./ConfirmationDialog.css";

type ConfirmationTone = "danger" | "neutral" | "question";

interface ConfirmationDialogProps {
  cancelAction: DialogAction;
  children?: ReactNode;
  confirmButtonRef?: Ref<HTMLButtonElement>;
  confirmAction: DialogAction;
  description: ReactNode;
  leadingAction?: DialogAction;
  title: string;
  tone?: ConfirmationTone;
}

export function ConfirmationDialog({
  cancelAction,
  children,
  confirmButtonRef,
  confirmAction,
  description,
  leadingAction,
  title,
  tone = "question",
}: ConfirmationDialogProps) {
  const icon = tone === "question" ? CircleHelp : CircleAlert;
  const titleId = useId();

  return (
    <DialogWindowFrame
      actions={
        <>
          {leadingAction ? (
            <ActionButton
              disabled={leadingAction.disabled}
              onClick={leadingAction.onClick}
            >
              {leadingAction.label}
            </ActionButton>
          ) : null}
          {leadingAction ? <span className="ui-dialog-action-spacer" /> : null}
          <ActionButton
            disabled={cancelAction.disabled}
            onClick={cancelAction.onClick}
          >
            {cancelAction.label}
          </ActionButton>
          <ActionButton
            className={
              tone === "danger"
                ? "ui-confirmation-dialog__danger-action"
                : undefined
            }
            disabled={confirmAction.disabled}
            onClick={confirmAction.onClick}
            ref={confirmButtonRef}
            variant={tone === "danger" ? "secondary" : "primary"}
          >
            {confirmAction.label}
          </ActionButton>
        </>
      }
      layout="message"
      title={title}
      titleId={titleId}
    >
      <div className="ui-standard-message" data-tone={tone}>
        {tone !== "neutral" ? (
          <span
            aria-hidden="true"
            className="ui-standard-message__icon"
            data-tone={tone}
          >
            <AppIcon icon={icon} size={14} />
          </span>
        ) : null}
        <div className="ui-standard-message__content">
          <h2 className="ui-standard-message__title" id={titleId}>
            {title}
          </h2>
          <div className="ui-standard-message__description">
            {description}
          </div>
          {children ? (
            <div className="ui-standard-message__extra">{children}</div>
          ) : null}
        </div>
      </div>
    </DialogWindowFrame>
  );
}
