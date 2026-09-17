import { Channel, invoke } from "@tauri-apps/api/core";
import type { ImageProcessingProgress } from "../application/projectPorts";
import type { ImageProcessingProgress as IpcImageProcessingProgress } from "../contracts/generated/ImageProcessingProgress";

export async function invokeImageProcessing<T>(
  command: string,
  args: Record<string, unknown>,
  onProgress?: (progress: ImageProcessingProgress) => void,
): Promise<T> {
  const progressChannel = new Channel<IpcImageProcessingProgress>();
  let active = true;
  progressChannel.onmessage = (progress) => { if (active) onProgress?.(progress); };
  try {
    return await invoke<T>(command, { ...args, onProgress: progressChannel });
  } finally {
    active = false;
  }
}
