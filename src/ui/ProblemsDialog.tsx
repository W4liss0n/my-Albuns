import { useRef, type ReactNode } from "react";

import { ActionButton } from "./ActionButton";
import { DialogFocusScope } from "./DialogFocusScope";
import { DialogWindowFrame } from "./DialogWindowFrame";
import "./ProblemsDialog.css";

/** One entry of the list: what is affected, what happened and what can be done. */
export interface ProblemsDialogItem {
  key: string;
  /** The affected project or file; one entry per project or per file. */
  title: ReactNode;
  /** Extra identification shown on hover, such as the project path. */
  titleHint?: string;
  /** One or more lines describing the problems of this entry. */
  details: ReactNode;
  actions?: ReactNode;
}

interface ProblemsDialogProps {
  title: string;
  description: string;
  items: readonly ProblemsDialogItem[];
  onClose(): void;
  closeDisabled?: boolean;
  closeLabel?: string;
  /** Actions for the whole list, beside the description. */
  toolbar?: ReactNode;
  /** Secondary actions on the left of the footer. */
  leadingActions?: ReactNode;
  /** The main action, after Close on the right of the footer. */
  primaryAction?: ReactNode;
}

export function ProblemsDialog({
  title,
  description,
  items,
  onClose,
  closeDisabled = false,
  closeLabel = "Fechar",
  toolbar,
  leadingActions,
  primaryAction,
}: ProblemsDialogProps) {
  const closeRef = useRef<HTMLButtonElement>(null);
  return (
    <DialogFocusScope
      focusKey={title}
      initialFocusRef={closeRef}
      onEscape={() => { if (!closeDisabled) onClose(); }}
    >
      <DialogWindowFrame
        title={title}
        layout="problems"
        actions={<>
          {leadingActions && <div className="ui-problems-leading-actions">{leadingActions}</div>}
          <ActionButton ref={closeRef} disabled={closeDisabled} onClick={onClose}>{closeLabel}</ActionButton>
          {primaryAction}
        </>}
      >
        <div className="ui-problems-intro">
          <p className="ui-problems-description">{description}</p>
          {toolbar && <div className="ui-problems-toolbar">{toolbar}</div>}
        </div>
        <div
          className="ui-problems-scroll"
          tabIndex={0}
          role="region"
          aria-label={title}
        >
          <ul className="ui-problems-list ui-copyable-text">
            {items.map(item => (
              <li key={item.key}>
                <div className="ui-problems-item">
                  <strong className="ui-problems-item__title" title={item.titleHint}>{item.title}</strong>
                  <div className="ui-problems-item__details">{item.details}</div>
                </div>
                {item.actions && <div className="ui-problems-item__actions">{item.actions}</div>}
              </li>
            ))}
          </ul>
        </div>
      </DialogWindowFrame>
    </DialogFocusScope>
  );
}
