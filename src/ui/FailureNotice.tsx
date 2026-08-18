import { InlineNotice } from "./InlineNotice";

interface ActionableFailure {
  action?: string;
  message: string;
}

interface FailureNoticeProps {
  failure: ActionableFailure;
  title: string;
}

export function FailureNotice({ failure, title }: FailureNoticeProps) {
  return (
    <InlineNotice role="alert" title={title} tone="error">
      <p>{failure.message}</p>
      {failure.action ? <p>{failure.action}</p> : null}
    </InlineNotice>
  );
}
