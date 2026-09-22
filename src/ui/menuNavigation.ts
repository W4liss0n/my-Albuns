type MenuFocusTarget = "first" | "last" | "next" | "previous" | "selected";

interface MenuFocusOptions {
  current?: EventTarget | null;
  preventScroll?: boolean;
  fallbackToContainer?: boolean;
}

/** Traverses one menu; nested menus and their key bindings belong to the caller. */
export function focusMenuItem(
  container: HTMLElement,
  target: MenuFocusTarget,
  {
    current = document.activeElement,
    preventScroll = false,
    fallbackToContainer = true,
  }: MenuFocusOptions = {},
) {
  const items = Array.from(
    container.querySelectorAll<HTMLElement>(
      '[role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"]',
    ),
  ).filter((item) =>
    !item.matches(':disabled, [aria-disabled="true"]') &&
    item.closest('[role="menu"]') === container,
  );
  const currentIndex = items.findIndex((item) => item === current);
  let item: HTMLElement | undefined;
  switch (target) {
    case "first": item = items[0]; break;
    case "last": item = items[items.length - 1]; break;
    case "selected":
      item = items.find((candidate) => candidate.dataset.selected === "true") ?? items[0];
      break;
    case "next": item = items[(currentIndex + 1) % items.length]; break;
    case "previous":
      item = items[(currentIndex <= 0 ? items.length : currentIndex) - 1];
      break;
  }
  const destination = item ?? (fallbackToContainer ? container : undefined);
  destination?.focus({ preventScroll });
}
