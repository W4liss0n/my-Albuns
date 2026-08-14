interface BrandWordmarkProps {
  compact?: boolean;
  subtitle?: string;
}

export function BrandWordmark({
  compact = false,
  subtitle,
}: BrandWordmarkProps) {
  return (
    <span
      aria-label="MyAlbuns"
      className={compact ? "ui-brand ui-brand--compact" : "ui-brand"}
    >
      <span aria-hidden="true">myalbuns</span>
      {subtitle ? <small>{subtitle}</small> : null}
    </span>
  );
}
