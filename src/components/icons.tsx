/**
 * Line icons drawn to a 16px grid at 1.5px stroke, matching the weight of the
 * system's own glyphs. Inline SVG rather than an icon package: eight shapes do
 * not justify a dependency, and these inherit `currentColor` cleanly.
 */

type IconProps = { size?: number; className?: string };

function Svg({ size = 16, className, children }: IconProps & { children: React.ReactNode }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
      focusable="false"
    >
      {children}
    </svg>
  );
}

export const StorageIcon = (props: IconProps) => (
  <Svg {...props}>
    <ellipse cx="8" cy="4" rx="5.5" ry="2.2" />
    <path d="M2.5 4v8c0 1.2 2.5 2.2 5.5 2.2s5.5-1 5.5-2.2V4" />
    <path d="M2.5 8c0 1.2 2.5 2.2 5.5 2.2s5.5-1 5.5-2.2" />
  </Svg>
);

export const CleanupIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M2.8 4.5h10.4" />
    <path d="M6.2 4.5V3.2a1 1 0 0 1 1-1h1.6a1 1 0 0 1 1 1v1.3" />
    <path d="M4.2 4.5l.6 8.2a1 1 0 0 0 1 .9h4.4a1 1 0 0 0 1-.9l.6-8.2" />
  </Svg>
);

export const StartupIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M8 2.2v6" />
    <path d="M4.6 4.3a5 5 0 1 0 6.8 0" />
  </Svg>
);

export const MemoryIcon = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3.2" y="3.2" width="9.6" height="9.6" rx="1.6" />
    <rect x="6" y="6" width="4" height="4" rx="0.8" />
    <path d="M6 1.6v1.6M10 1.6v1.6M6 12.8v1.6M10 12.8v1.6M1.6 6h1.6M1.6 10h1.6M12.8 6h1.6M12.8 10h1.6" />
  </Svg>
);

export const FolderIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M2 5.2V12a1.2 1.2 0 0 0 1.2 1.2h9.6A1.2 1.2 0 0 0 14 12V6.4a1.2 1.2 0 0 0-1.2-1.2H8.3L6.9 3.6A1.2 1.2 0 0 0 6 3.2H3.2A1.2 1.2 0 0 0 2 4.4z" />
  </Svg>
);

export const FileIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M9.2 2H4.6a1.2 1.2 0 0 0-1.2 1.2v9.6A1.2 1.2 0 0 0 4.6 14h6.8a1.2 1.2 0 0 0 1.2-1.2V5.2z" />
    <path d="M9.2 2v3.2h3.4" />
  </Svg>
);

export const LinkIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M6.6 9.4a2.6 2.6 0 0 0 3.9.3l1.8-1.8a2.6 2.6 0 0 0-3.7-3.7l-1 1" />
    <path d="M9.4 6.6a2.6 2.6 0 0 0-3.9-.3L3.7 8.1a2.6 2.6 0 0 0 3.7 3.7l1-1" />
  </Svg>
);

export const ChevronRightIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M6.2 3.4 10.8 8l-4.6 4.6" />
  </Svg>
);

export const LockIcon = (props: IconProps) => (
  <Svg {...props}>
    <rect x="3.6" y="7" width="8.8" height="6.4" rx="1.4" />
    <path d="M5.8 7V5.2a2.2 2.2 0 0 1 4.4 0V7" />
  </Svg>
);
