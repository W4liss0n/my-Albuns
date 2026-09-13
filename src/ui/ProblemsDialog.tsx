import { useRef, type ReactNode } from "react";

import { ActionButton } from "./ActionButton";
import { DialogFocusScope } from "./DialogFocusScope";
import { DialogWindowFrame } from "./DialogWindowFrame";
import "./ProblemsDialog.css";

interface ProblemsDialogProps {
  title: string;
  description: string;
  columns: readonly string[];
  rows: readonly (readonly ReactNode[])[];
  onClose(): void;
  closeDisabled?: boolean;
  closeLabel?: string;
  actions?: ReactNode;
}

export function ProblemsDialog({
  title,
  description,
  columns,
  rows,
  onClose,
  closeDisabled = false,
  closeLabel = "Fechar",
  actions,
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
        actions={<>{actions}<ActionButton ref={closeRef} disabled={closeDisabled} onClick={onClose}>{closeLabel}</ActionButton></>}
      >
        <p className="ui-problems-description">{description}</p>
        <div
          className="ui-problems-scroll"
          tabIndex={0}
          role="region"
          aria-label={title}
        >
          <table className="ui-problems-table ui-copyable-text">
            <thead>
              <tr>{columns.map(column => (
                <th scope="col" key={column}>{column}</th>
              ))}</tr>
            </thead>
            <tbody>
              {rows.map((cells, rowIndex) => (
                <tr key={rowIndex}>
                  {cells.map((cell, columnIndex) => <td key={columnIndex}>{cell}</td>)}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </DialogWindowFrame>
    </DialogFocusScope>
  );
}
