export interface RecentProjectOpeningTime {
  label: string;
  fullLabel: string;
  dateTime: string;
}

const localTime = new Intl.DateTimeFormat("pt-BR", {
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});
const localDate = new Intl.DateTimeFormat("pt-BR", {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
});

function sameLocalDay(left: Date, right: Date): boolean {
  return left.getFullYear() === right.getFullYear() &&
    left.getMonth() === right.getMonth() &&
    left.getDate() === right.getDate();
}

export function recentProjectOpeningTime(
  lastOpenedAtMs: number | null,
  now: Date = new Date(),
): RecentProjectOpeningTime | null {
  if (lastOpenedAtMs === null || !Number.isSafeInteger(lastOpenedAtMs) ||
    lastOpenedAtMs < 0) return null;
  const openedAt = new Date(lastOpenedAtMs);
  if (!Number.isFinite(openedAt.getTime())) return null;

  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1);
  const day = sameLocalDay(openedAt, now)
    ? "Hoje"
    : sameLocalDay(openedAt, yesterday)
      ? "Ontem"
      : localDate.format(openedAt);
  return {
    label: day,
    fullLabel: `${day} às ${localTime.format(openedAt)}`,
    dateTime: openedAt.toISOString(),
  };
}
