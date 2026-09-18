import { useEffect, useId, useMemo, useRef, useState } from "react";
import { Dialog, Popover } from "react-aria-components";
import type { MediaFolder, MediaFolderEdit, MediaFolderNameError, MediaFolderNameRequest, MediaKind } from "../domain/project";
import type { ProjectCorePort } from "../application/projectPorts";
import { ActionButton, TextInput } from "../ui";
import { FieldValidationAutoTooltip, FieldValidationTooltip, fieldValidationTooltipAttributes, useFieldValidationTooltip } from "../ui/FieldValidationTooltip";

export type MediaFolderPrompt = {
  anchor: HTMLElement;
  mediaKind: MediaKind;
} & ({ kind: "create" } | { kind: "rename"; folder: MediaFolder }
  | { kind: "move"; mediaIds: string[]; folderId: string | null });

const nameMessages: Record<MediaFolderNameError, string> = {
  empty: "Informe o nome da pasta.",
  invalidCharactersOrLength: "Use até 80 caracteres, sem quebras de linha.",
  nameInUse: "Esse nome já está em uso nesta aba.",
  folderNotFound: "A pasta não existe mais.",
};
const validationUnavailable = "Não foi possível validar o nome. Tente novamente.";

export function MediaFolderPopover({ prompt, folders, validationKey, onValidate, onSubmit, onClose }: {
  prompt: MediaFolderPrompt;
  folders: readonly MediaFolder[];
  validationKey: string;
  onValidate: ProjectCorePort["validateMediaFolderName"];
  onSubmit(edit: MediaFolderEdit): Promise<boolean>;
  onClose(): void;
}) {
  const anchor = useRef(prompt.anchor);
  // Let the context menu release its focus-outside listener before autofocus.
  const [open, setOpen] = useState(false);
  useEffect(() => { setOpen(true); }, []);
  const input = useRef<HTMLInputElement>(null);
  const running = useRef(false);
  const validationSequence = useRef(0);
  const [pending, setPending] = useState(false);
  const [name, setName] = useState(prompt.kind === "rename" ? prompt.folder.name : "");
  const [folderId, setFolderId] = useState(prompt.kind === "move" ? prompt.folderId ?? "" : "");
  const [attempted, setAttempted] = useState(false);
  const [validated, setValidated] = useState<{ key: object; error: string | null } | null>(null);
  const mounted = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const titleId = useId();
  const fieldId = useId();
  const title = prompt.kind === "create" ? "Nova pasta" : prompt.kind === "rename" ? "Renomear pasta" : "Mover para pasta";
  const sameKind = folders.filter((folder) => folder.kind === prompt.mediaKind);
  const request: MediaFolderNameRequest = { mediaKind: prompt.mediaKind, name,
    folderId: prompt.kind === "rename" ? prompt.folder.id : null };
  const key = useMemo(() => ({}), [validationKey, prompt, name, folders]);
  const currentKey = useRef(key);
  currentKey.current = key;
  const error = validated?.key === key ? validated.error : null;

  useEffect(() => {
    if (!attempted || prompt.kind === "move" || running.current) return;
    let active = true;
    const sequence = ++validationSequence.current;
    void onValidate(request).then((result) => {
      if (active && sequence === validationSequence.current && currentKey.current === key) setValidated({ key, error: result.error ? nameMessages[result.error] : null });
    }).catch(() => {
      if (active && sequence === validationSequence.current && currentKey.current === key) setValidated({ key, error: validationUnavailable });
    });
    return () => { active = false; };
  }, [key, attempted, onValidate]);

  const tooltip = useFieldValidationTooltip(`${fieldId}-error`, attempted && prompt.kind !== "move" && error ? [{ field: "name", messages: [error] }] : []);

  useEffect(() => {
    if (!pending && error) input.current?.focus();
  }, [pending, error, validated]);

  async function submit() {
    if (running.current) return;
    running.current = true;
    ++validationSequence.current;
    setPending(true);
    try {
      let edit: MediaFolderEdit;
      if (prompt.kind === "move") {
        edit = { kind: "moveMedia", mediaIds: prompt.mediaIds, folderId: folderId || null };
      } else {
        setAttempted(true);
        let result;
        try { result = await onValidate(request); }
        catch {
          if (mounted.current && currentKey.current === key) setValidated({ key, error: validationUnavailable });
          return;
        }
        if (!mounted.current || currentKey.current !== key) return;
        setValidated({ key, error: result.error ? nameMessages[result.error] : null });
        if (result.error) return;
        edit = prompt.kind === "create" ? { kind: "create", mediaKind: prompt.mediaKind, name: result.name }
          : { kind: "rename", folderId: prompt.folder.id, name: result.name };
      }
      if (await onSubmit(edit)) onClose();
    } finally {
      running.current = false;
      setPending(false);
      if (mounted.current && currentKey.current === key) {
        tooltip.show("name");
      }
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
          value={name} {...fieldValidationTooltipAttributes("name", attempted ? error ?? undefined : undefined, tooltip)}
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
