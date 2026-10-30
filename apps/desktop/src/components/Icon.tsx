// Stroke icons (24 × 24, currentColor).

const PATHS: Record<string, string> = {
  logo: "M12 3l8 4.5v9L12 21l-8-4.5v-9z M12 12l8-4.5 M12 12v9 M12 12L4 7.5",
  search: "M11 4a7 7 0 1 0 0 14a7 7 0 1 0 0-14z M20 20l-3.5-3.5",
  plus: "M12 5v14 M5 12h14",
  settings: "M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6z M12 2.5v3 M12 18.5v3 M2.5 12h3 M18.5 12h3 M5.3 5.3l2.1 2.1 M16.6 16.6l2.1 2.1 M5.3 18.7l2.1-2.1 M16.6 7.4l2.1-2.1",
  layers: "M12 3l9 5-9 5-9-5 9-5z M3 13l9 5 9-5",
  star: "M12 3.5l2.6 5.3 5.9.9-4.3 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.3-4.1 5.9-.9z",
  clock: "M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 1 0 0-17z M12 7.5V12l3 2",
  sparkle: "M12 3v4 M12 17v4 M3 12h4 M17 12h4 M6 6l2.5 2.5 M15.5 15.5L18 18 M6 18l2.5-2.5 M15.5 8.5L18 6",
  trash: "M4 7h16 M10 11v6 M14 11v6 M6 7l1 13h10l1-13 M9 7V4h6v3",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  tag: "M3 12V4h8l10 10-8 8z M7.5 7.3a1.2 1.2 0 1 0 0 2.4a1.2 1.2 0 1 0 0-2.4z",
  grid: "M4 4h7v7H4z M13 4h7v7h-7z M4 13h7v7H4z M13 13h7v7h-7z",
  list: "M9 6h11 M9 12h11 M9 18h11 M4.5 6h.01 M4.5 12h.01 M4.5 18h.01",
  back: "M15 18l-6-6 6-6",
  chevronDown: "M6 9l6 6 6-6",
  chevronRight: "M9 6l6 6-6 6",
  undo: "M9 14L4 9l5-5 M4 9h10a6 6 0 0 1 0 12h-3",
  redo: "M15 14l5-5-5-5 M20 9H10a6 6 0 0 0 0 12h3",
  history: "M3 12a9 9 0 1 0 3-6.7L3 8 M3 3v5h5 M12 7v5l3 2",
  download: "M12 4v11 M7 10l5 5 5-5 M5 20h14",
  upload: "M12 20V9 M7 14l5-5 5 5 M5 4h14",
  eye: "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6z",
  eyeOff: "M3 3l18 18 M10.6 5.1A10.4 10.4 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3 3.8 M6.6 6.6C3.8 8.4 2 12 2 12s3.5 7 10 7a9.7 9.7 0 0 0 4.4-1.1 M9.9 9.9a3 3 0 0 0 4.2 4.2",
  check: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M8 12.5l2.5 2.5L16 9.5",
  alert: "M12 3l9.5 17h-19z M12 10v4 M12 17h.01",
  info: "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18z M12 11v5 M12 8h.01",
  home: "M4 11l8-7 8 7v9h-5v-6h-6v6H4z",
  fit: "M4 9V4h5 M20 9V4h-5 M4 15v5h5 M20 15v5h-5",
  ruler: "M3 17L17 3l4 4L7 21z M7 13l2 2 M10 10l2 2 M13 7l2 2",
  section: "M3 12h18 M6 8l6-4 6 4 M6 16l6 4 6-4",
  plate: "M3 3h18v18H3z M3 9h18 M3 15h18 M9 3v18 M15 3v18",
  more: "M5 12h.01 M12 12h.01 M19 12h.01",
  close: "M6 6l12 12 M18 6L6 18",
  copy: "M8 8h12v12H8z M16 8V4H4v12h4",
  paste: "M9 4h6v3H9z M7 5H5v16h14V5h-2",
  file: "M6 3h8l4 4v14H6z M14 3v4h4",
  box: "M12 3l8 4.5v9L12 21l-8-4.5v-9z",
  restore: "M3 12a9 9 0 1 0 3-6.7L3 8 M3 3v5h5",
  edit: "M4 20h4L19 9l-4-4L4 16z M14 6l4 4",
  sun: "M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8z M12 2v2 M12 20v2 M2 12h2 M20 12h2 M4.9 4.9l1.4 1.4 M17.7 17.7l1.4 1.4 M4.9 19.1l1.4-1.4 M17.7 6.3l1.4-1.4",
  keyboard: "M3 6h18v12H3z M7 10h.01 M11 10h.01 M15 10h.01 M7 14h10",
  printer: "M6 9V3h12v6 M6 18H4v-9h16v9h-2 M8 14h8v7H8z",
};

export function Icon({ name, size = 20, className, style, filled }: { name: keyof typeof PATHS | string; size?: number; className?: string; style?: React.CSSProperties; filled?: boolean }) {
  const d = PATHS[name] ?? PATHS.box;
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={filled ? "currentColor" : "none"}
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      className={className}
      style={style}
    >
      <path d={d} />
    </svg>
  );
}
