import {
  PROJECT_CONFIGURATION_VALIDATION_CODES,
  type ProjectConfigurationValidationCode,
  type ProjectConfigurationValidationOutcome,
  type ProjectLaunchFailure,
  type ProjectLaunchOutcome,
  type ProvisionalDecorativeSelection,
} from "../application/globalProjectPort";
import { hasOnlyIpcKeys, isIpcRecord } from "../../platform/ipcGuards";

const validationCodes = new Set<ProjectConfigurationValidationCode>(
  PROJECT_CONFIGURATION_VALIDATION_CODES,
);

function parseProjectLaunchFailure(
  error: unknown,
): ProjectLaunchFailure | null {
  if (!isIpcRecord(error)) return null;
  const candidate = error;
  if (
    typeof candidate.code !== "string" ||
    typeof candidate.message !== "string"
  ) {
    return null;
  }
  return {
    code: candidate.code,
    ...(typeof candidate.stage === "string"
      ? { stage: candidate.stage }
      : {}),
    message: candidate.message,
    ...(typeof candidate.action === "string"
      ? { action: candidate.action }
      : {}),
  };
}

export function toProjectLaunchFailure(
  error: unknown,
  fallback: ProjectLaunchFailure,
): ProjectLaunchFailure {
  return parseProjectLaunchFailure(error) ?? fallback;
}

export function parseProjectLaunchOutcome(
  value: unknown,
): ProjectLaunchOutcome | null {
  if (!isIpcRecord(value) || typeof value.status !== "string") {
    return null;
  }
  if (
    (value.status === "opened" ||
      value.status === "focused" ||
      value.status === "externalCopyNotWritable" ||
      value.status === "cancelled") &&
    hasOnlyIpcKeys(value, ["status"])
  ) {
    return { status: value.status };
  }
  if (
    value.status === "failed" &&
    hasOnlyIpcKeys(value, ["status", "error"])
  ) {
    const error = parseProjectLaunchFailure(value.error);
    return error ? { status: "failed", error } : null;
  }
  return null;
}

export function toProjectLaunchOutcome(
  result: unknown,
  fallback: ProjectLaunchFailure,
): ProjectLaunchOutcome {
  return parseProjectLaunchOutcome(result) ?? {
    status: "failed",
    error: fallback,
  };
}

export async function settleProjectLaunch(
  attempt: () => Promise<unknown>,
  fallback: ProjectLaunchFailure,
): Promise<ProjectLaunchOutcome> {
  try {
    return toProjectLaunchOutcome(await attempt(), fallback);
  } catch (error) {
    return {
      status: "failed",
      error: toProjectLaunchFailure(error, fallback),
    };
  }
}

export async function settleConfigurationValidation(
  attempt: () => Promise<unknown>,
  fallback: ProjectLaunchFailure,
): Promise<ProjectConfigurationValidationOutcome> {
  try {
    const result = await attempt();
    if (typeof result !== "object" || result === null) {
      return { status: "failed", error: fallback };
    }
    const candidate = result as Record<string, unknown>;
    if (!Array.isArray(candidate.errors)) {
      return { status: "failed", error: fallback };
    }
    const errors = candidate.errors.flatMap((code) =>
      typeof code === "string" &&
      validationCodes.has(code as ProjectConfigurationValidationCode)
        ? [code as ProjectConfigurationValidationCode]
        : [],
    );
    if (errors.length !== candidate.errors.length) {
      return { status: "failed", error: fallback };
    }
    return errors.length === 0
      ? { status: "valid" }
      : { status: "invalid", errors };
  } catch (error) {
    return {
      status: "failed",
      error: toProjectLaunchFailure(error, fallback),
    };
  }
}

export function toProvisionalDecorativeSelection(
  result: unknown,
): ProvisionalDecorativeSelection | null {
  if (result === null || typeof result !== "object") return null;
  const candidate = result as Record<string, unknown>;
  if (
    typeof candidate.selectionId !== "string" ||
    candidate.selectionId.length === 0 ||
    candidate.selectionId.includes("/") ||
    typeof candidate.displayName !== "string" ||
    candidate.displayName.length === 0 ||
    typeof candidate.previewUrl !== "string" ||
    !(
      candidate.previewUrl.startsWith(
        "http://myalbuns-preview.localhost/",
      ) || candidate.previewUrl.startsWith("myalbuns-preview://localhost/")
    )
  ) {
    return null;
  }
  return {
    selectionId: candidate.selectionId,
    displayName: candidate.displayName,
    previewUrl: candidate.previewUrl,
  };
}
