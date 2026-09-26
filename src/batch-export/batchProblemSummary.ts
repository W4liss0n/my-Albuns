import type { BatchProblem } from "../contracts/generated/BatchProblem";

export interface BatchProblemLine {
  text: string;
  /** Full list of files when the line shows only the first ones. */
  hint?: string;
}

const groupedKinds = {
  missingMedia: { one: "Imagem ausente", many: "imagens ausentes" },
  unavailable: { one: "Imagem indisponível", many: "imagens indisponíveis" },
} as const;

const shownFiles = 3;

/**
 * Lines for one Project on the Problems screen. Image problems of the same kind
 * become a single line ("2 imagens ausentes: 001.jpg, 014.jpg"); other problems
 * keep their own message, without repeating an identical one. Lines follow the
 * order in which each kind first appears.
 */
export function summarizeBatchProblems(problems: readonly BatchProblem[]): BatchProblemLine[] {
  const entries: ({ kind: keyof typeof groupedKinds; files: string[] } | { text: string })[] = [];
  for (const problem of problems) {
    const kind = problem.kind;
    if (problem.fileName && (kind === "missingMedia" || kind === "unavailable")) {
      const group = entries.find(entry => "kind" in entry && entry.kind === kind);
      if (group && "files" in group) group.files.push(problem.fileName);
      else entries.push({ kind, files: [problem.fileName] });
    } else if (!entries.some(entry => "text" in entry && entry.text === problem.message)) {
      entries.push({ text: problem.message });
    }
  }
  return entries.map(entry => {
    if ("text" in entry) return { text: entry.text };
    const label = groupedKinds[entry.kind];
    const { files } = entry;
    if (files.length === 1) return { text: `${label.one}: ${files[0]}` };
    const listed = files.slice(0, shownFiles).join(", ");
    const hidden = files.length - shownFiles;
    return hidden > 0
      ? { text: `${files.length} ${label.many}: ${listed} e mais ${hidden}`, hint: files.join(", ") }
      : { text: `${files.length} ${label.many}: ${listed}` };
  });
}
