import { X } from "lucide-react";
import { useLayoutEffect, useRef } from "react";
import { AppIcon } from "../ui";

export function LayoutCatalogNotice({ message, onDismiss }: {
  message: string;
  onDismiss(): void;
}) {
  const noticeRef = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    noticeRef.current?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  }, [message]);

  return (
    <div className="ui-anchored-tooltip layout-catalog-notice" ref={noticeRef} role="status">
      <span>{message}</span>
      <button aria-label="Fechar aviso" onClick={onDismiss} type="button">
        <AppIcon icon={X} size={12} />
      </button>
    </div>
  );
}
