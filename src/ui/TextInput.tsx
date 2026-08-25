import {
  forwardRef,
  type ComponentPropsWithoutRef,
} from "react";

export type TextInputProps = Omit<
  ComponentPropsWithoutRef<"input">,
  "autoComplete"
>;

/**
 * Product text belongs to application state, never to the browser/WebView
 * autofill history.
 */
export const TextInput = forwardRef<HTMLInputElement, TextInputProps>(
  function TextInput(props, ref) {
    return <input {...props} autoComplete="off" ref={ref} />;
  },
);
