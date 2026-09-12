export interface CacheSettingsStatus {
  occupiedBytes: number;
  releasableBytes: number;
  clearAllScheduled: boolean;
}

export interface CacheSettingsPort {
  status(): Promise<CacheSettingsStatus>;
  freeClosedProjects(): Promise<{ freedBytes: number }>;
  clearAll(): Promise<{ kind: "cleared"; result: { freedBytes: number } } | { kind: "scheduled" }>;
}
