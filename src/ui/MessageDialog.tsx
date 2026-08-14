import type { ReactNode } from "react";
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
  title: string;
  tone: MessageTone;
}

export function MessageDialog({
  description,
  detail,
  primaryAction,
  secondaryAction,
  title,
  tone,
}: MessageDialogProps) {
  const icon = tone === "error" ? CircleX : CircleCheck;
  const actions =
    primaryAction || secondaryAction ? (
      <>
        {secondaryAction ? (
          <ActionButton
            disabled={secondaryAction.disabled}
            onClick={secondaryAction.onClick}
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
    <DialogWindowFrame actions={actions} layout="message" title={title}>
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
          <h2 className="ui-standard-message__title">{title}</h2>
          <div className="ui-standard-message__description">
            {description}
          </div>
          {detail ? (
            <div className="ui-standard-message__detail">{detail}</div>
          ) : null}
        </div>
      </div>
    </DialogWindowFrame>
  );
}
