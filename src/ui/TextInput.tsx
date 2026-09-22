import {
  forwardRef,
  type ComponentPropsWithoutRef,
} from "react";
import "./TextInput.css";

export type TextInputProps = Omit<
  ComponentPropsWithoutRef<"input">,
  "autoComplete"
> & { appearance?: "standard" | "integrated" };

/**
 * Product text belongs to application state, never to the browser/WebView
 * autofill history.
 */
export const TextInput = forwardRef<HTMLInputElement, TextInputProps>(
  function TextInput({ appearance = "standard", className, ...props }, ref) {
    const fieldClassName = appearance === "integrated"
      ? ["ui-field-control", "ui-text-input--integrated", className].filter(Boolean).join(" ")
      : className;
    return <input {...props} className={fieldClassName} autoComplete="off" ref={ref} />;
  },
);
