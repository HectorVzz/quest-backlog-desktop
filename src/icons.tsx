// Small stroke icons on a 24 px grid. Drawn here instead of pulling in an
// icon package: the panel needs about a dozen.

import type { StatusKey } from "./i18n";

const PATHS = {
  // Statuses
  playing: "M8 5.5v13l10.5-6.5z",
  completed: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM8.5 12.2l2.4 2.4 4.6-4.9",
  platinumed:
    "M8 20h8M12 16v4M7 4h10v5a5 5 0 0 1-10 0zM17 6h2.5v1.5A3.5 3.5 0 0 1 16.4 11M7 6H4.5v1.5A3.5 3.5 0 0 0 7.6 11",
  backlog: "M12 3.5l8.5 4.25L12 12 3.5 7.75zM3.5 12l8.5 4.25L20.5 12M3.5 16.25L12 20.5l8.5-4.25",
  dropped: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM5.7 5.7l12.6 12.6",
  wishlist: "M7 3.5h10a1 1 0 0 1 1 1v16l-6-4-6 4v-16a1 1 0 0 1 1-1z",
  // Interface
  close: "M6 6l12 12M18 6L6 18",
  settings: "M4 7h10M18 7h2M4 17h4M12 17h8M16 5v4M10 15v4",
  camera:
    "M4 8.5A1.5 1.5 0 0 1 5.5 7h2l1.5-2.5h6L16.5 7h2A1.5 1.5 0 0 1 20 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 17.5zM12 16a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
  window: "M4 5.5A1.5 1.5 0 0 1 5.5 4h13A1.5 1.5 0 0 1 20 5.5v13a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 18.5zM4 9h16",
  crop: "M7 3v14h14M3 7h14v14",
  trash: "M5 7h14M10 11v6M14 11v6M6.5 7l1 12.5h9l1-12.5M9.5 7V4.5h5V7",
  search: "M10.5 17a6.5 6.5 0 1 0 0-13 6.5 6.5 0 0 0 0 13zM15.5 15.5L20 20",
  check: "M5 12.5l4.5 4.5L19 7.5",
  arrow: "M5 12h14M13 6l6 6-6 6",
  back: "M19 12H5M11 6l-6 6 6 6",
  refresh: "M19.5 12a7.5 7.5 0 1 1-2.2-5.3M19.5 4v4h-4",
  heart: "M12 19.5s-7.5-4.6-7.5-10A4.25 4.25 0 0 1 12 6.8a4.25 4.25 0 0 1 7.5 2.7c0 5.4-7.5 10-7.5 10z",
  comment: "M5 5.5h14a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1h-8l-4.5 3.5v-3.5H5a1 1 0 0 1-1-1v-9a1 1 0 0 1 1-1z",
  external: "M14 4h6v6M20 4l-9 9M18 14v4.5a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 4 18.5v-11A1.5 1.5 0 0 1 5.5 6H10",
} as const;

export type IconName = keyof typeof PATHS;

export function Icon({
  name,
  size = 18,
  filled = false,
}: {
  name: IconName | StatusKey;
  size?: number;
  /** Fill the shape too, for an "on" state such as a liked heart. */
  filled?: boolean;
}) {
  return (
    <svg
      class="icon"
      viewBox="0 0 24 24"
      width={size}
      height={size}
      aria-hidden="true"
      fill={filled ? "currentColor" : "none"}
      stroke="currentColor"
      stroke-width="1.8"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}

/** The app mark: the app icon (src-tauri/icons/icon.svg) in theme colors. */
export function Mark({ size = 18 }: { size?: number }) {
  return (
    <svg class="mark" viewBox="64 64 896 896" width={size} height={size} aria-hidden="true">
      <rect x="64" y="64" width="896" height="896" rx="208" fill="var(--qb-color-accent)" />
      <path
        d="M376 224h272a40 40 0 0 1 40 40v536L512 678 336 800V264a40 40 0 0 1 40-40z"
        fill="var(--qb-color-on-accent)"
      />
      <path
        d="M424 448l64 64 120-128"
        fill="none"
        stroke="var(--qb-color-accent)"
        stroke-width="60"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  );
}
