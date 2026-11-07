import type { ReactNode } from "react";

type IconProps = { width?: number; height?: number; title?: string };

function svg(props: IconProps, children: ReactNode) {
  const { width = 18, height = 18, title } = props;
  return (
    <svg
      width={width}
      height={height}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.8}
      strokeLinecap="round"
      strokeLinejoin="round"
      role={title ? "img" : undefined}
      aria-label={title}
      aria-hidden={title ? undefined : true}
      focusable="false"
    >
      {children}
    </svg>
  );
}

export const JobsIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <path d="M7 9h10M7 13h6" />
    </>,
  );

export const BookIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
      <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2Z" />
    </>,
  );

export const WrongNoteIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <path d="M6 3h12a1 1 0 0 1 1 1v17l-7-4-7 4V4a1 1 0 0 1 1-1z" />
      <path d="M12 7v5M12 15.5v.5" />
    </>,
  );

export const SettingsIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21h-4v-.1a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3v-4h.1a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3 1.7 1.7 0 0 0 1-1.5V3h4v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.5 1h.1v4h-.1a1.7 1.7 0 0 0-1.5 1z" />
    </>,
  );

export const UserIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <circle cx="12" cy="8" r="3.5" />
      <path d="M5 20a7 7 0 0 1 14 0" />
    </>,
  );

export const LogoutIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
      <path d="m16 17 5-5-5-5" />
      <path d="M21 12H9" />
    </>,
  );

export const SearchIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <circle cx="11" cy="11" r="7" />
      <path d="m20 20-3.5-3.5" />
    </>,
  );

export const RefreshIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <path d="M20 11a8 8 0 1 0-2.3 6" />
      <path d="M20 4v7h-7" />
    </>,
  );

export const InboxIcon = (p: IconProps) =>
  svg(
    p,
    <path d="M4 13l2.5-7h11L20 13v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1v-5Zm0 0h5l1 2h4l1-2h5" />,
  );

export const AlertIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <path d="M12 3 2 21h20L12 3Z" />
      <path d="M12 10v4" />
      <path d="M12 17.5v.01" />
    </>,
  );

export const CheckIcon = (p: IconProps) => svg(p, <path d="m20 6-11 11-5-5" />);

export const UploadIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <path d="M12 16V4" />
      <path d="m7 9 5-5 5 5" />
      <path d="M5 20h14" />
    </>,
  );

export const TrashIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <path d="M4 7h16" />
      <path d="M10 11v6M14 11v6" />
      <path d="M6 7l1 13a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-13" />
      <path d="M9 7V4h6v3" />
    </>,
  );

export const ChevronLeftIcon = (p: IconProps) =>
  svg(p, <path d="m15 18-6-6 6-6" />);

export const ChevronRightIcon = (p: IconProps) =>
  svg(p, <path d="m9 18 6-6-6-6" />);

export const ArrowLeftIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <path d="M19 12H5" />
      <path d="m11 6-6 6 6 6" />
    </>,
  );

export const PlusIcon = (p: IconProps) =>
  svg(
    p,
    <>
      <path d="M12 5v14" />
      <path d="M5 12h14" />
    </>,
  );
