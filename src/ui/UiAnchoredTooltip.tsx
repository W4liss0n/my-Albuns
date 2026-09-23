import { useRef, type RefObject } from "react";
import { createPortal } from "react-dom";
import { useOverlayPosition, useTooltip, useTooltipTrigger, type Placement } from "react-aria";
import { useTooltipTriggerState } from "react-stately";
import "./UiAnchoredTooltip.css";

export function useUiAnchoredTooltip(targetRef: RefObject<HTMLElement | null>, label: string, disabled = false, preferredPlacement: Placement = "top") {
  const overlayRef = useRef<HTMLDivElement>(null);
  const arrowRef = useRef<HTMLSpanElement>(null);
  const options = { isDisabled: disabled, delay: 600, closeDelay: 100 };
  const state = useTooltipTriggerState(options);
  const { triggerProps, tooltipProps: descriptionProps } = useTooltipTrigger(options, state, targetRef);
  const { tooltipProps } = useTooltip(descriptionProps, state);
  const { overlayProps, arrowProps, placement } = useOverlayPosition({
    targetRef, overlayRef, arrowRef, arrowSize: 9, arrowBoundaryOffset: 8,
    isOpen: state.isOpen, placement: preferredPlacement,
    offset: 8, containerPadding: 12, shouldFlip: true,
  });
  return {
    triggerProps,
    tooltip: state.isOpen && createPortal(
      <div {...tooltipProps} {...overlayProps} ref={overlayRef} className="ui-anchored-tooltip ui-floating-tooltip" data-placement={placement}>
        <span {...arrowProps} ref={arrowRef} className="ui-floating-tooltip__arrow" aria-hidden="true" />
        {label}
      </div>, document.body,
    ),
  };
}
