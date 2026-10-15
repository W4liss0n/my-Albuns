import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { MediaFolder, MediaFolderNameError, MediaKind } from "../../domain/project";
import type { ProjectCorePort } from "../../application/projectPorts";
import { TextInput } from "../../ui";
import { FieldValidationTooltip, fieldValidationTooltipAttributes, useFieldValidationTooltip } from "../../ui/FieldValidationTooltip";

const DEFAULT_NAME = "Nova coleção";
const nameMessages: Record<MediaFolderNameError, string> = {
  empty: "Informe o nome da coleção.",
  invalidCharactersOrLength: "Use até 80 caracteres, sem quebras de linha.",
  nameInUse: "Esse nome já está em uso nesta aba.",
  folderNotFound: "A coleção não existe mais.",
};
const validationUnavailable = "Não foi possível validar o nome. Tente novamente.";

/** "Nova coleção", or the first free "Nova coleção (n)" in the tab, as Windows names a new folder. */
export function nextCollectionName(folders: readonly MediaFolder[]) {
  const taken = new Set(folders.map((folder) => folder.name.toLocaleLowerCase("pt-BR")));
  if (!taken.has(DEFAULT_NAME.toLocaleLowerCase("pt-BR"))) return DEFAULT_NAME;
  for (let number = 2; ; number += 1) {
    const candidate = `${DEFAULT_NAME} (${number})`;
    if (!taken.has(candidate.toLocaleLowerCase("pt-BR"))) return candidate;
  }
}

interface MediaFolderNameEditorProps {
  mediaKind: MediaKind;
  /** The renamed collection; `null` while naming a new one. */
  folder: MediaFolder | null;
  /** Name a new collection keeps when the person leaves it unnamed. */
  defaultName: string;
  validationKey: string;
  onValidate: ProjectCorePort["validateMediaFolderName"];
  onCommit(name: string): Promise<boolean>;
  onCancel(): void;
}

/**
 * Names a collection in place, like a file in Explorer: Enter or leaving the
 * field keeps the name, an invalid name stays open with its tooltip after
 * Enter. A new collection is never discarded: Esc or an empty name keeps the
 * default name. Renaming with Esc, an empty or an invalid name on leaving
 * restores the current name.
 */
export function MediaFolderNameEditor({ mediaKind, folder, defaultName, validationKey, onValidate, onCommit, onCancel }: MediaFolderNameEditorProps) {
  const [name, setName] = useState(folder?.name ?? defaultName);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const running = useRef(false);
  const finished = useRef(false);
  const mounted = useRef(false);
  const input = useRef<HTMLInputElement>(null);
  const fieldId = useId();
  const tooltip = useFieldValidationTooltip(`${fieldId}-error`, error ? [{ field: "name", messages: [error] }] : []);
  const chip = useRef<HTMLSpanElement>(null);
  const [tooltipAnchor, setTooltipAnchor] = useState<{ left: number; top: number } | null>(null);
  const tooltipOpen = tooltip.openField === "name" && Boolean(tooltip.message);
  // The chip strip scrolls sideways and would clip a tooltip placed inside it.
  useLayoutEffect(() => {
    const bounds = tooltipOpen ? chip.current?.getBoundingClientRect() : undefined;
    setTooltipAnchor(bounds ? { left: bounds.left, top: bounds.bottom + 8 } : null);
  }, [tooltipOpen]);

  useEffect(() => {
    mounted.current = true;
    // Focus after the commit, not with autoFocus: a closing menu still listens
    // for focus leaving it during the commit and would steal it back.
    input.current?.focus();
    input.current?.select();
    return () => { mounted.current = false; };
  }, []);
  // A stale error no longer describes the name once the project changes.
  const currentKey = useRef(validationKey);
  useEffect(() => { currentKey.current = validationKey; setError(null); }, [validationKey]);

  // Disabling the field while saving drops its focus; a failed save returns it
  // once the field is enabled again, so Enter and Esc keep working.
  const refocus = useRef(false);
  useEffect(() => {
    if (pending || !refocus.current) return;
    refocus.current = false;
    input.current?.focus();
  }, [pending]);

  async function commit(nextName: string) {
    setPending(true);
    const saved = await onCommit(nextName);
    if (saved) { finished.current = true; return; }
    if (mounted.current) {
      refocus.current = true;
      setPending(false);
    }
  }

  async function finish(reason: "enter" | "blur" | "escape") {
    if (running.current || finished.current) return;
    running.current = true;
    try {
      const typed = name.trim();
      const fallback = folder ? null : defaultName;
      const keep = async () => {
        if (fallback === null) { finished.current = true; onCancel(); }
        else await commit(fallback);
      };
      if (reason === "escape" || typed === "" || (folder && typed === folder.name)) return await keep();
      let result;
      try {
        // Advice from an older document is asked again, never trusted.
        for (let attempt = 0; ; attempt += 1) {
          const key = currentKey.current;
          result = await onValidate({ mediaKind, name: typed, folderId: folder?.id ?? null });
          if (key === currentKey.current || attempt === 2) break;
        }
      } catch {
        if (reason === "blur") return await keep();
        if (mounted.current) { setError(validationUnavailable); tooltip.show("name"); }
        return;
      }
      if (!result.error) return await commit(result.name);
      if (reason === "blur") return await keep();
      if (mounted.current) {
        setError(nameMessages[result.error]);
        tooltip.show("name");
        input.current?.focus();
      }
    } finally {
      running.current = false;
    }
  }

  return <span ref={chip} className="media-folder-chip media-folder-chip--editing">
    <FieldValidationTooltip tooltip={tooltip} />
    <TextInput ref={input} id={fieldId} aria-label={folder ? `Nome da coleção ${folder.name}` : "Nome da nova coleção"}
      className="media-folder-name-input" disabled={pending} value={name}
      {...fieldValidationTooltipAttributes("name", error ?? undefined, tooltip)}
      onChange={(event) => { setName(event.target.value); if (error) setError(null); }}
      onBlur={() => { void finish("blur"); }}
      onKeyDown={(event) => {
        // Album and panel shortcuts never act while a name is being typed.
        event.stopPropagation();
        if (event.key === "Enter") { event.preventDefault(); void finish("enter"); }
        else if (event.key === "Escape") { event.preventDefault(); void finish("escape"); }
      }} />
    {tooltipAnchor && createPortal(<span className="ui-anchored-tooltip ui-field-validation-auto-tooltip media-folder-name-tooltip" role="tooltip"
      style={tooltipAnchor}>{tooltip.message}</span>, document.body)}
  </span>;
}
