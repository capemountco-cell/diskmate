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

export const MacIcon = (props: IconProps) => (
  <Svg {...props}>
    <rect x="2" y="2.8" width="12" height="8" rx="1.4" />
    <path d="M5.6 13.6h4.8M8 10.8v2.8" />
  </Svg>
);

export const DuplicatesIcon = (props: IconProps) => (
  <Svg {...props}>
    <rect x="5.4" y="5.4" width="8.4" height="8.4" rx="1.4" />
    <path d="M10.8 5.4V3.6a1.2 1.2 0 0 0-1.2-1.2H3.6a1.2 1.2 0 0 0-1.2 1.2v6a1.2 1.2 0 0 0 1.2 1.2h1.8" />
  </Svg>
);

export const ApplicationsIcon = (props: IconProps) => (
  <Svg {...props}>
    <rect x="2.4" y="2.4" width="4.6" height="4.6" rx="1.2" />
    <rect x="9" y="2.4" width="4.6" height="4.6" rx="1.2" />
    <rect x="2.4" y="9" width="4.6" height="4.6" rx="1.2" />
    <rect x="9" y="9" width="4.6" height="4.6" rx="1.2" />
  </Svg>
);

export const ChevronLeftIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M9.8 3.4 5.2 8l4.6 4.6" />
  </Svg>
);

export const BoxIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M8 1.8 2.6 4.6v6.8L8 14.2l5.4-2.8V4.6z" />
    <path d="M2.6 4.6 8 7.4l5.4-2.8M8 7.4v6.8" />
  </Svg>
);

export const CodeIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M5.4 4.4 1.8 8l3.6 3.6M10.6 4.4 14.2 8l-3.6 3.6M9.2 2.6l-2.4 10.8" />
  </Svg>
);

export const DownloadIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M8 2.2v7.6M4.8 6.8 8 10l3.2-3.2" />
    <path d="M2.4 11.2v1.2a1.2 1.2 0 0 0 1.2 1.2h8.8a1.2 1.2 0 0 0 1.2-1.2v-1.2" />
  </Svg>
);

export const ClockIcon = (props: IconProps) => (
  <Svg {...props}>
    <circle cx="8" cy="8" r="5.8" />
    <path d="M8 4.6V8l2.2 1.4" />
  </Svg>
);

export const DocumentIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M9.2 2H4.6a1.2 1.2 0 0 0-1.2 1.2v9.6A1.2 1.2 0 0 0 4.6 14h6.8a1.2 1.2 0 0 0 1.2-1.2V5.2z" />
    <path d="M9.2 2v3.2h3.4M5.8 8.4h4.4M5.8 10.8h4.4" />
  </Svg>
);

export const MapIcon = (props: IconProps) => (
  <Svg {...props}>
    <rect x="2" y="2" width="12" height="12" rx="1.6" />
    <path d="M7 2v12M7 8h7M10.6 8v6" />
  </Svg>
);

export const RankIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M2.6 4h10.8M2.6 8h7.2M2.6 12h3.6" />
  </Svg>
);

export const TrendIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M2 11.6 6 7.6l2.6 2.6L14 4.8" />
    <path d="M10.4 4.8H14v3.6" />
  </Svg>
);

export const LeftBehindIcon = (props: IconProps) => (
  <Svg {...props}>
    <path d="M2.6 4.4h10.8M2.6 8h6.4M2.6 11.6h4" />
    <path d="m10.4 10.4 3.2 3.2M13.6 10.4l-3.2 3.2" />
  </Svg>
);
