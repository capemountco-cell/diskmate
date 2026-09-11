/**
 * Small building blocks used across pages.
 *
 * These stay deliberately thin: each one is markup plus a class name, so the
 * visual language lives in CSS where it can be read as a whole rather than
 * scattered through component files.
 */

import type { ReactNode } from "react";

export function Panel({
  title,
  note,
  action,
  flush,
  children,
}: {
  title?: ReactNode;
  note?: ReactNode;
  action?: ReactNode;
  flush?: boolean;
  children: ReactNode;
}) {
  return (
    <section className="panel">
      {(title || action) && (
        <header className="panel__header">
          <div>
            {title && <h2 className="panel__title">{title}</h2>}
            {note && <p className="panel__note">{note}</p>}
          </div>
          {action}
        </header>
      )}
      <div className={flush ? "panel__body panel__body--flush" : "panel__body"}>{children}</div>
    </section>
  );
}

export function Stat({
  label,
  value,
  hint,
  small,
}: {
  label: string;
  value: ReactNode;
  hint?: ReactNode;
  small?: boolean;
}) {
  return (
    <div>
      <div className="stat__label">{label}</div>
      <div className={small ? "stat__value stat__value--small" : "stat__value"}>{value}</div>
      {hint && <div className="stat__hint">{hint}</div>}
    </div>
  );
}

export function Button({
  variant = "default",
  children,
  ...props
}: { variant?: "default" | "primary" | "quiet" } & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  const className = ["button", variant !== "default" ? `button--${variant}` : ""]
    .filter(Boolean)
    .join(" ");
  return (
    <button type="button" className={className} {...props}>
      {children}
    </button>
  );
}

export function EmptyState({ title, children }: { title: string; children?: ReactNode }) {
  return (
    <div className="empty-state">
      <p className="empty-state__title">{title}</p>
      {children && <p className="empty-state__body">{children}</p>}
    </div>
  );
}

/**
 * A note. DiskMate's only device for "you should know this".
 *
 * There is intentionally no `danger` tone: nothing this product reports is an
 * emergency, and styling ordinary facts as alarms is how cleaner apps train
 * people to panic.
 */
export function Note({
  tone = "info",
  title,
  children,
}: {
  tone?: "info" | "caution";
  title?: string;
  children: ReactNode;
}) {
  return (
    <div className={`note note--${tone}`} role="note">
      <div>
        {title && <span className="note__title">{title} </span>}
        <span>{children}</span>
      </div>
    </div>
  );
}

export function Spinner() {
  return <span className="spinner" role="progressbar" aria-label="Loading" />;
}

export function Badge({
  tone = "neutral",
  children,
}: {
  tone?: "neutral" | "accent" | "caution";
  children: ReactNode;
}) {
  return <span className={tone === "neutral" ? "badge" : `badge badge--${tone}`}>{children}</span>;
}
