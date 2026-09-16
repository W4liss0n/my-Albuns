import { useEffect, useId, useRef, useState } from "react";
import { Dialog, Popover } from "react-aria-components";
import type { MediaFolder, MediaFolderEdit, MediaKind } from "../domain/project";
import { ActionButton, TextInput } from "../ui";
import { FieldValidationAutoTooltip, FieldValidationTooltip, fieldValidationTooltipAttributes, useFieldValidationTooltip } from "../ui/FieldValidationTooltip";

export type MediaFolderPrompt = {
  anchor: HTMLElement;
  mediaKind: MediaKind;
} & ({ kind: "create" } | { kind: "rename"; folder: MediaFolder }
  | { kind: "move"; mediaIds: string[]; folderId: string | null });

export function MediaFolderPopover({ prompt, folders, onSubmit, onClose }: {
  prompt: MediaFolderPrompt;
  folders: readonly MediaFolder[];
  onSubmit(edit: MediaFolderEdit): Promise<boolean>;
  onClose(): void;
}) {
  const anchor = useRef(prompt.anchor);
  // Let the context menu release its focus-outside listener before autofocus.
  const [open, setOpen] = useState(false);
  useEffect(() => { setOpen(true); }, []);
  const input = useRef<HTMLInputElement>(null);
  const running = useRef(false);
  const [pending, setPending] = useState(false);
  const [name, setName] = useState(prompt.kind === "rename" ? prompt.folder.name : "");
  const [folderId, setFolderId] = useState(prompt.kind === "move" ? prompt.folderId ?? "" : "");
  const [attempted, setAttempted] = useState(false);
  const titleId = useId();
  const fieldId = useId();
  const title = prompt.kind === "create" ? "Nova pasta" : prompt.kind === "rename" ? "Renomear pasta" : "Mover para pasta";
  const sameKind = folders.filter((folder) => folder.kind === prompt.mediaKind);
  const trimmed = name.trim();
  const error = !trimmed ? "Informe o nome da pasta." : [...trimmed].length > 80 || /[\u0000-\u001f\u007f-\u009f]/u.test(trimmed)
    ? "Use até 80 caracteres, sem quebras de linha."
    : ["todas", "ausentes"].includes(trimmed.toLowerCase()) || sameKind.some((folder) =>
      folder.name.toLowerCase() === trimmed.toLowerCase() && (prompt.kind !== "rename" || folder.id !== prompt.folder.id))
      ? "Esse nome já está em uso nesta aba." : null;

  const tooltip = useFieldValidationTooltip(`${fieldId}-error`, attempted && prompt.kind !== "move" && error ? [{ field: "name", messages: [error] }] : []);

  async function submit() {
    if (running.current) return;
    if (prompt.kind !== "move" && error) {
      setAttempted(true);
      input.current?.focus();
      tooltip.show("name");
      return;
    }
    running.current = true;
    setPending(true);
    const edit: MediaFolderEdit = prompt.kind === "move"
      ? { kind: "moveMedia", mediaIds: prompt.mediaIds, folderId: folderId || null }
      : prompt.kind === "create" ? { kind: "create", mediaKind: prompt.mediaKind, name: trimmed }
        : { kind: "rename", folderId: prompt.folder.id, name: trimmed };
    try {
      if (await onSubmit(edit)) onClose();
    } finally {
      running.current = false;
      setPending(false);
    }
  }

  return <Popover className="ui-floating-surface media-folder-popover" isOpen={open} triggerRef={anchor}
    placement="top start" offset={8} isKeyboardDismissDisabled={pending}
    shouldCloseOnInteractOutside={() => !pending}
    onOpenChange={(open) => { if (!open && !running.current) onClose(); }}>
    <Dialog aria-labelledby={titleId} className="media-folder-dialog">
      <form noValidate onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === "Escape") { event.preventDefault(); if (!pending) onClose(); }
      }} onSubmit={(event) => { event.preventDefault(); void submit(); }}>
        <h3 id={titleId}>{title}</h3>
        <FieldValidationTooltip tooltip={tooltip} />
        <label htmlFor={fieldId}>{prompt.kind === "move" ? "Destino" : "Nome"}</label>
        {prompt.kind === "move" ? <select id={fieldId} autoFocus className="ui-field-control" disabled={pending}
          value={folderId} onChange={(event) => setFolderId(event.target.value)}>
          <option value="">Sem pasta</option>
          {sameKind.map((folder) => <option key={folder.id} value={folder.id}>{folder.name}</option>)}
        </select> : <span className="media-folder-name-field"><TextInput id={fieldId} ref={input} autoFocus className="ui-field-control" disabled={pending}
          value={name} maxLength={160} {...fieldValidationTooltipAttributes("name", attempted ? error ?? undefined : undefined, tooltip)}
          onChange={(event) => { setName(event.target.value); }}
          onFocus={(event) => { event.target.select(); if (attempted && error) tooltip.show("name"); }} />
          <FieldValidationAutoTooltip field="name" tooltip={tooltip} /></span>}
        <div className="media-folder-dialog-actions">
          <ActionButton density="compact" disabled={pending} onClick={onClose}>Cancelar</ActionButton>
          <ActionButton density="compact" variant="primary" type="submit" disabled={pending}>
            {prompt.kind === "create" ? "Criar" : prompt.kind === "rename" ? "Salvar" : "Mover"}
          </ActionButton>
        </div>
      </form>
    </Dialog>
  </Popover>;
}
