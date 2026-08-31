import { useId, type ReactNode, type Ref } from "react";
import { CircleCheck, CircleX } from "lucide-react";

import { ActionButton } from "./ActionButton";
import { AppIcon } from "./AppIcon";
import type { DialogAction } from "./DialogAction";
import { DialogWindowFrame } from "./DialogWindowFrame";

type MessageTone = "error" | "success";

interface MessageDialogProps {
  description: ReactNode;
  detail?: ReactNode;
  primaryAction?: DialogAction;
  secondaryAction?: DialogAction;
  secondaryButtonRef?: Ref<HTMLButtonElement>;
  title: string;
  tone: MessageTone;
}

export function MessageDialog({
  description,
  detail,
  primaryAction,
  secondaryAction,
  secondaryButtonRef,
  title,
  tone,
}: MessageDialogProps) {
  const icon = tone === "error" ? CircleX : CircleCheck;
  const titleId = useId();
  const actions =
    primaryAction || secondaryAction ? (
      <>
        {secondaryAction ? (
          <ActionButton
            disabled={secondaryAction.disabled}
            onClick={secondaryAction.onClick}
            ref={secondaryButtonRef}
          >
            {secondaryAction.label}
          </ActionButton>
        ) : null}
        {primaryAction ? (
          <ActionButton
            disabled={primaryAction.disabled}
            onClick={primaryAction.onClick}
            variant="primary"
          >
            {primaryAction.label}
          </ActionButton>
        ) : null}
      </>
    ) : undefined;

  return (
    <DialogWindowFrame
      actions={actions}
      layout="message"
      title={title}
      titleId={titleId}
    >
      <div
        aria-live={tone === "error" ? "assertive" : "polite"}
        className="ui-standard-message"
        data-tone={tone}
        role={tone === "error" ? "alert" : "status"}
      >
        <span
          aria-hidden="true"
          className="ui-standard-message__icon"
          data-tone={tone}
        >
          <AppIcon icon={icon} size={14} />
        </span>
        <div className="ui-standard-message__content">
          <h2 className="ui-standard-message__title" id={titleId}>
            {title}
          </h2>
          <div
            className={
              tone === "error"
                ? "ui-standard-message__description ui-copyable-text"
                : "ui-standard-message__description"
            }
          >
            {description}
          </div>
          {detail ? (
            <div className="ui-standard-message__detail ui-copyable-text">
              {detail}
            </div>
          ) : null}
        </div>
      </div>
    </DialogWindowFrame>
  );
}
