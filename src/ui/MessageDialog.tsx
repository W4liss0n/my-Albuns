import { useId, type ReactNode, type Ref } from "react";

import { ActionButton } from "./ActionButton";
import type { DialogAction } from "./DialogAction";
import { DialogWindowFrame } from "./DialogWindowFrame";
import { useDefaultDialogAction } from "./useDefaultDialogAction";

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
  const titleId = useId();
  // The primary action is the highlighted one; a lone action is the default.
  const defaultActionRef = useDefaultDialogAction(
    title,
    primaryAction ? undefined : secondaryButtonRef,
  );
  const actions =
    primaryAction || secondaryAction ? (
      <>
        {secondaryAction ? (
          <ActionButton
            disabled={secondaryAction.disabled}
            onClick={secondaryAction.onClick}
            ref={primaryAction ? secondaryButtonRef : defaultActionRef}
          >
            {secondaryAction.label}
          </ActionButton>
        ) : null}
        {primaryAction ? (
          <ActionButton
            disabled={primaryAction.disabled}
            onClick={primaryAction.onClick}
            ref={defaultActionRef}
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
