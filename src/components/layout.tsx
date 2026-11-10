/**
 * The building blocks of the section screens.
 *
 * Every section follows one of two shapes. Before it has anything to show it
 * is an introduction: what the section does, and one button. Once it has
 * results it is a headline sentence over a grid of cards, each card one finding
 * with one number. These components are those shapes, so the pages only decide
 * what to say.
 */

import type { ReactNode } from "react";

import { ChevronLeftIcon } from "./icons";

/** The slim bar at the top of a results screen. */
export function PageHeader({
  title,
  back,
  aside,
}: {
  title: string;
  back?: { label: string; onClick: () => void };
  aside?: ReactNode;
}) {
  return (
    <header className="page-header">
      <div>
        {back && (
          <button type="button" className="page-header__back" onClick={back.onClick}>
            <ChevronLeftIcon size={16} />
            {back.label}
          </button>
        )}
      </div>
      <h1 className="page-header__title">{title}</h1>
      <div className="page-header__aside">{aside}</div>
    </header>
  );
}

/** A large page title for dashboards, where there is no single headline. */
export function DisplayTitle({ title, aside }: { title: string; aside?: ReactNode }) {
  return (
    <header className="display-title">
      <h1 className="display-title__text">{title}</h1>
      {aside && <div className="display-title__aside">{aside}</div>}
    </header>
  );
}

/** The one sentence a results screen leads with. */
export function Hero({
  title,
  subtitle,
  action,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <section className="hero">
      <h2 className="hero__title">{title}</h2>
      {subtitle && <p className="hero__subtitle">{subtitle}</p>}
      {action && <div className="hero__action">{action}</div>}
    </section>
  );
}

/** A glass tile holding an icon, used as a card's emblem. */
export function GlassTile({
  children,
  size = "md",
}: {
  children: ReactNode;
  size?: "md" | "lg";
}) {
  return (
    <span className={`glass-tile glass-tile--${size}`} aria-hidden="true">
      {children}
    </span>
  );
}

/**
 * One finding.
 *
 * `layout` lets the first two cards of a grid take more room, the way a
 * newspaper gives the lead story more of the page.
 */
export function FindingCard({
  title,
  body,
  emblem,
  footnote,
  actions,
  layout = "normal",
  children,
}: {
  title: ReactNode;
  body?: ReactNode;
  emblem?: ReactNode;
  footnote?: ReactNode;
  actions?: ReactNode;
  layout?: "normal" | "tall" | "wide";
  children?: ReactNode;
}) {
  return (
    <article className={`finding finding--${layout}`}>
      <div className="finding__head">
        <div className="finding__text">
          <h3 className="finding__title">{title}</h3>
          {body && <p className="finding__body">{body}</p>}
        </div>
        {emblem && layout !== "tall" && emblem}
      </div>
      {children}
      {layout === "tall" && emblem && <div className="finding__art">{emblem}</div>}
      <div className="finding__foot">
        <span className="finding__footnote">{footnote}</span>
        <span className="finding__actions">{actions}</span>
      </div>
    </article>
  );
}

/** Where each card in a findings grid goes, given how many there are. */
export function findingLayout(index: number, count: number): "normal" | "tall" | "wide" {
  // Chosen so a three-column grid has no holes for the common counts.
  if (count <= 2) return index === 0 ? "wide" : "normal";
  if (index === 0) return "tall";
  if (index === 1) return "wide";
  if (count === 3) return "wide";
  return "normal";
}

/**
 * The introduction a section shows before it has looked at anything.
 *
 * The button is the only way forward, and the line under it says exactly what
 * pressing it will and will not do.
 */
export function IntroScreen({
  title,
  subtitle,
  art,
  features,
  action,
  footnote,
  children,
}: {
  title: string;
  subtitle: string;
  art: ReactNode;
  features: Array<{ icon: ReactNode; label: string }>;
  action: { label: string; onClick: () => void; disabled?: boolean };
  footnote: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="intro">
      <div className="intro__art">{art}</div>
      <div className="intro__copy">
        <h1 className="intro__title">{title}</h1>
        <p className="intro__subtitle">{subtitle}</p>
        <ul className="intro__features">
          {features.map((feature) => (
            <li key={feature.label} className="intro__feature">
              <span className="intro__feature-icon" aria-hidden="true">
                {feature.icon}
              </span>
              {feature.label}
            </li>
          ))}
        </ul>
        {children}
      </div>
      <div className="intro__go">
        <button
          type="button"
          className="scan-button"
          onClick={action.onClick}
          disabled={action.disabled}
        >
          {action.label}
        </button>
        <div className="intro__footnote">{footnote}</div>
      </div>
    </div>
  );
}

/** A card on a dashboard: a label, one number, and what it was measured from. */
export function MetricCard({
  label,
  value,
  unit,
  measured,
  children,
  className,
}: {
  label: string;
  value: ReactNode;
  unit?: ReactNode;
  measured?: ReactNode;
  children?: ReactNode;
  className?: string;
}) {
  return (
    <article className={className ? `metric ${className}` : "metric"}>
      <div className="metric__label">{label}</div>
      <div className="metric__value">
        {value}
        {unit && <span className="metric__unit"> {unit}</span>}
      </div>
      {measured && <div className="metric__measured">{measured}</div>}
      {children}
    </article>
  );
}
