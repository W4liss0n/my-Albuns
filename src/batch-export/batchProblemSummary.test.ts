import { expect, test } from "vitest";

import type { BatchProblem } from "../contracts/generated/BatchProblem";
import { summarizeBatchProblems } from "./batchProblemSummary";

const missing = (fileName: string): BatchProblem => ({ kind: "missingMedia", message: `Imagem ausente: ${fileName}`, mediaId: fileName, fileName });
const unavailable = (fileName: string): BatchProblem => ({ kind: "unavailable", message: `Imagem indisponível: ${fileName}`, mediaId: fileName, fileName });

test("keeps a single image problem in the singular", () => {
  expect(summarizeBatchProblems([missing("001.jpg")])).toEqual([{ text: "Imagem ausente: 001.jpg" }]);
});

test("groups images of the same kind and keeps kinds and other problems apart, in order", () => {
  const placeholder: BatchProblem = { kind: "placeholder", message: "Preencha os quadros vazios e salve o projeto.", mediaId: null, fileName: null };
  expect(summarizeBatchProblems([missing("001.jpg"), placeholder, unavailable("020.jpg"), missing("014.jpg"), placeholder])).toEqual([
    { text: "2 imagens ausentes: 001.jpg, 014.jpg" },
    { text: "Preencha os quadros vazios e salve o projeto." },
    { text: "Imagem indisponível: 020.jpg" },
  ]);
});

test("shows the first three files and counts the rest, with the full list as a hint", () => {
  const files = ["001.jpg", "003.jpg", "007.jpg", "009.jpg", "011.jpg"];
  expect(summarizeBatchProblems(files.map(missing))).toEqual([{
    text: "5 imagens ausentes: 001.jpg, 003.jpg, 007.jpg e mais 2",
    hint: "001.jpg, 003.jpg, 007.jpg, 009.jpg, 011.jpg",
  }]);
});

test("falls back to the message when a checkpoint has no file name", () => {
  const legacy: BatchProblem = { kind: "missingMedia", message: "Imagem ausente: 001.jpg", mediaId: "photo", fileName: null };
  expect(summarizeBatchProblems([legacy, legacy])).toEqual([{ text: "Imagem ausente: 001.jpg" }]);
});
