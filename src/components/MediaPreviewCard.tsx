import type {
  ButtonHTMLAttributes,
  ReactNode,
} from "react";

import type { MediaCatalogItem } from "../domain/project";
import { MediaThumbnail } from "./MediaThumbnail";
import "./MediaPreviewCard.css";

interface MediaPreviewCardBaseProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children"> {
  children?: ReactNode;
}

type MediaPreviewCardProps =
  | (MediaPreviewCardBaseProps & {
      dimmed?: boolean;
      kind: "media";
      loading?: "eager" | "lazy";
      media: Pick<MediaCatalogItem, "sourceHeightPx" | "sourceWidthPx">;
      previewUrl?: string;
      selected: boolean;
    })
  | (MediaPreviewCardBaseProps & {
      kind: "placeholder";
    });

interface MediaPreviewCardButtonProps extends MediaPreviewCardBaseProps {
  appearance: "media" | "placeholder";
  dimmed?: boolean;
  selected: boolean;
}

function MediaPreviewCardButton({
  appearance,
  children,
  className,
  dimmed,
  selected,
  type = "button",
  ...buttonProps
}: MediaPreviewCardButtonProps) {
  return (
    <button
      {...buttonProps}
      className={[
        "media-preview-card",
        `media-preview-card--${appearance}`,
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      data-dimmed={dimmed ? "true" : undefined}
      data-selected={String(selected)}
      type={type}
    >
      {children}
    </button>
  );
}

/**
 * Owns the interactive media-card protocol shared by the media panel and
 * decorative pickers. The discriminated placeholder mode represents the one
 * non-media option that participates in those grids without exposing the
 * thumbnail implementation to callers.
 */
export function MediaPreviewCard(props: MediaPreviewCardProps) {
  if (props.kind === "placeholder") {
    const { children, kind: _kind, ...buttonProps } = props;
    return (
      <MediaPreviewCardButton
        {...buttonProps}
        appearance="placeholder"
        selected={false}
      >
        <span aria-hidden="true" className="media-preview-card__placeholder">
          {children}
        </span>
      </MediaPreviewCardButton>
    );
  }

  const {
    children,
    dimmed,
    kind: _kind,
    loading,
    media,
    previewUrl,
    selected,
    ...buttonProps
  } = props;
  return (
    <MediaPreviewCardButton
      {...buttonProps}
      appearance="media"
      dimmed={dimmed}
      selected={selected}
    >
      <MediaThumbnail
        loading={loading}
        media={media}
        previewUrl={previewUrl}
      >
        {children}
      </MediaThumbnail>
    </MediaPreviewCardButton>
  );
}
