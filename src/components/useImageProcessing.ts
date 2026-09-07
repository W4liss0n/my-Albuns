import { useEffect, useRef, useState } from "react";
import type { ImageProcessingProblem, ImageProcessingProgress } from "../application/projectPorts";

const NO_PROBLEMS: readonly ImageProcessingProblem[] = [];

export function useImageProcessing(projectId: string, operationContext: object) {
  const [progress, setProgress] = useState<ImageProcessingProgress | null>(null);
  const [problems, setProblems] = useState(NO_PROBLEMS);
  const context = useRef({});
  useEffect(() => {
    context.current = {};
    setProgress(null);
    setProblems(NO_PROBLEMS);
    return () => { context.current = {}; };
  }, [projectId, operationContext]);

  async function run<T>(operation: (publish: (progress: ImageProcessingProgress) => void) => Promise<T>): Promise<T> {
    const attempt = {};
    context.current = attempt;
    try {
      return await operation((next) => {
        if (context.current !== attempt) return;
        setProgress(next);
        if (next.problem) {
          const problem = next.problem;
          setProblems((current) => [...current, problem]);
        }
      });
    } finally {
      if (context.current === attempt) {
        context.current = {};
        setProgress(null);
      }
    }
  }

  return { progress, problems, run, dismissProblems: () => setProblems(NO_PROBLEMS) };
}
