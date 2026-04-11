import { type ReactNode } from "react";

export interface AvatarProps {
  name: string;
  email?: string;
  src?: string;
  size?: "xs" | "sm" | "md" | "lg";
  ring?: boolean;
  onClick?: () => void;
  ariaLabel?: string;
  trailing?: ReactNode;
}

const sizeMap = {
  xs: { box: "w-6 h-6", text: "var(--text-2xs)" },
  sm: { box: "w-8 h-8", text: "var(--text-xs)" },
  md: { box: "w-10 h-10", text: "var(--text-sm)" },
  lg: { box: "w-14 h-14", text: "var(--text-lg)" },
};

// Deterministic gradient from a string — warm, ember-adjacent hues only.
function gradientFor(seed: string): string {
  let hash = 0;
  for (let i = 0; i < seed.length; i++) {
    hash = (hash * 31 + seed.charCodeAt(i)) | 0;
  }
  const palette = [
    ["#e94560", "#c73e54"],
    ["#f0a500", "#c78600"],
    ["#a78bfa", "#7c5ce0"],
    ["#6495ed", "#4770c4"],
    ["#4ecca3", "#2f8c72"],
    ["#e86a33", "#b94b1f"],
  ];
  const pair = palette[Math.abs(hash) % palette.length];
  return `linear-gradient(135deg, ${pair[0]}, ${pair[1]})`;
}

function initials(name: string): string {
  const parts = name.trim().split(/[\s._-]+/).filter(Boolean);
  if (parts.length === 0) return "?";
  if (parts.length === 1) return parts[0].slice(0, 2).toUpperCase();
  return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
}

export function Avatar({
  name,
  src,
  size = "md",
  ring = false,
  onClick,
  ariaLabel,
  trailing,
}: AvatarProps) {
  const s = sizeMap[size];
  const interactive = onClick !== undefined;

  const content = src ? (
    <img src={src} alt={name} className="w-full h-full object-cover" />
  ) : (
    <span
      className="w-full h-full flex items-center justify-center font-semibold text-white"
      style={{ background: gradientFor(name), fontSize: s.text }}
    >
      {initials(name)}
    </span>
  );

  const box = (
    <span
      className={`relative inline-flex items-center justify-center overflow-hidden rounded-full shrink-0 ${s.box} ${
        ring ? "ring-2 ring-border-accent ring-offset-2 ring-offset-surface" : ""
      }`}
    >
      {content}
      {trailing}
    </span>
  );

  if (interactive) {
    return (
      <button
        type="button"
        onClick={onClick}
        aria-label={ariaLabel ?? name}
        className="inline-flex items-center cursor-pointer rounded-full focus:outline-none focus-visible:shadow-[var(--ring-ember)]"
      >
        {box}
      </button>
    );
  }
  return box;
}
