import {
  useLayoutEffect,
  useRef,
  type KeyboardEvent,
  type ReactNode,
  type RefObject,
} from "react";

import { ConfirmationDialog, InlineNotice } from "../ui";

import "./ProjectRecoveryDialog.css";

const FOCUSABLE_SELECTOR = [
  "button:not(:disabled)",
  "[href]",
  "input:not(:disabled)",
  "select:not(:disabled)",
  "textarea:not(:disabled)",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

type ProjectRecoveryDialogState =
  | "available"
  | "confirmDiscard"
  | "resolving";

export function ProjectRecoveryDialog({
  error,
  onBack,
  onDefer,
  onDiscard,
  onRecover,
  onRequestDiscard,
  state,
}: {
  error: string | null;
  onBack(): void;
  onDefer(): void;
  onDiscard(): void;
  onRecover(): void;
  onRequestDiscard(): void;
  state: ProjectRecoveryDialogState;
}) {
  const primaryActionRef = useRef<HTMLButtonElement>(null);
  const busy = state === "resolving";

  if (state === "confirmDiscard") {
    return (
      <RecoveryDialogFocusScope
        focusKey={state}
        initialFocusRef={primaryActionRef}
        onEscape={onBack}
      >
        <ConfirmationDialog
          cancelAction={{ label: "Voltar", onClick: onBack }}
          confirmAction={{
            label: "Descartar recuperação e abrir",
            onClick: onDiscard,
          }}
          confirmButtonRef={primaryActionRef}
          description="A última versão salva será aberta e o trabalho recuperável será removido definitivamente."
          title="Descartar o trabalho recuperável?"
          tone="danger"
        >
          {error ? <InlineNotice tone="error">{error}</InlineNotice> : null}
        </ConfirmationDialog>
      </RecoveryDialogFocusScope>
    );
  }

  return (
    <RecoveryDialogFocusScope
      focusKey={state}
      initialFocusRef={primaryActionRef}
      onEscape={() => {
        if (!busy) onDefer();
      }}
    >
      <ConfirmationDialog
        cancelAction={{
          disabled: busy,
          label: "Abrir última versão salva",
          onClick: onRequestDiscard,
        }}
        confirmAction={{
          disabled: busy,
          label: "Reabrir e recuperar",
          onClick: onRecover,
        }}
        confirmButtonRef={primaryActionRef}
        description="O MyAlbuns encontrou trabalho concluído depois da última versão salva deste Projeto."
        leadingAction={{
          disabled: busy,
          label: "Agora não",
          onClick: onDefer,
        }}
        title="Recuperar trabalho não salvo?"
      >
        {error ? <InlineNotice tone="error">{error}</InlineNotice> : null}
        {busy ? <p aria-live="polite">Concluindo…</p> : null}
      </ConfirmationDialog>
    </RecoveryDialogFocusScope>
  );
}

function RecoveryDialogFocusScope({
  children,
  focusKey,
  initialFocusRef,
  onEscape,
}: {
  children: ReactNode;
  focusKey: string;
  initialFocusRef: RefObject<HTMLElement | null>;
  onEscape(): void;
}) {
  const scopeRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    initialFocusRef.current?.focus({ preventScroll: true });
  }, [focusKey, initialFocusRef]);

  const focusableElements = () =>
    Array.from(
      scopeRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR) ?? [],
    ).filter((element) => !element.hidden);

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      onEscape();
      return;
    }
    if (event.key !== "Tab") return;
    const focusable = focusableElements();
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (!first || !last) {
      event.preventDefault();
      return;
    }
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus({ preventScroll: true });
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus({ preventScroll: true });
    }
  };

  return (
    <div
      className="project-recovery-dialog-scope"
      onKeyDown={handleKeyDown}
      ref={scopeRef}
    >
      {children}
    </div>
  );
}
