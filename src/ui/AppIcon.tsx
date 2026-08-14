import type { LucideIcon } from "lucide-react";

interface AppIconProps {
  icon: LucideIcon;
  label?: string;
  size?: 12 | 14 | 16 | 18;
}

export function AppIcon({
  icon: Icon,
  label,
  size = 14,
}: AppIconProps) {
  return (
    <Icon
      aria-hidden={label ? undefined : "true"}
      aria-label={label}
      className="ui-app-icon"
      size={size}
      strokeWidth={1.4}
    />
  );
}
