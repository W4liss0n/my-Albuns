import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

import {
  renderUiAcceptanceReport,
  validateUiAcceptanceReview,
} from "./UiAcceptance.mjs";

const scriptsDirectory = path.dirname(fileURLToPath(import.meta.url));
const workspace = path.resolve(scriptsDirectory, "..");
const reviewPath = path.resolve(
  workspace,
  process.argv[2] ?? ".scratch/ui-acceptance/review.json",
);
const artifactDirectory = path.dirname(reviewPath);
const evidencePath = path.join(artifactDirectory, "evidence.json");
const reportPath = path.join(artifactDirectory, "review-report.html");

const evidence = JSON.parse(readFileSync(evidencePath, "utf8"));
const review = JSON.parse(readFileSync(reviewPath, "utf8"));
validateUiAcceptanceReview(evidence, review);
writeFileSync(reportPath, renderUiAcceptanceReport(evidence, review), "utf8");

const counts = Object.fromEntries(
  ["accepted", "rejected", "unvalidated"].map((outcome) => [
    outcome,
    review.scenarios.filter((decision) => decision.outcome === outcome).length,
  ]),
);
console.log(`Reviewed UI acceptance evidence: ${reportPath}`);
console.log(
  `${counts.accepted} accepted, ${counts.rejected} rejected, ${counts.unvalidated} unvalidated`,
);
if (counts.rejected > 0 || counts.unvalidated > 0) process.exitCode = 1;
