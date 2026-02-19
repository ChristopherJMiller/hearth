/**
 * Format an ISO timestamp as a human-readable relative time string.
 * Examples: "just now", "2m ago", "1h ago", "3d ago", "Feb 12"
 */
export function formatRelativeTime(iso: string): string {
  const date = new Date(iso);
  const now = Date.now();
  const diffMs = now - date.getTime();

  if (diffMs < 0) return 'just now';

  const seconds = Math.floor(diffMs / 1000);
  if (seconds < 60) return 'just now';

  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;

  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;

  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;

  return date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

/**
 * Format an ISO timestamp as a readable date-time string.
 * Example: "Feb 12, 2026 3:45 PM"
 */
export function formatDateTime(iso: string): string {
  return new Date(iso).toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });
}

/**
 * Truncate a string to the given max length with an ellipsis.
 */
export function truncate(str: string, max: number): string {
  if (str.length <= max) return str;
  return str.slice(0, max) + '...';
}

/**
 * Truncate a Nix store path to show just the hash prefix + name.
 * e.g. "/nix/store/abc123...-nixos-system-24.05" -> "abc123...-nixos-system-24.05"
 */
export function truncateStorePath(path: string): string {
  const parts = path.split('/');
  const last = parts[parts.length - 1] ?? path;
  if (last.length <= 40) return last;
  return last.slice(0, 12) + '...' + last.slice(-20);
}

/**
 * Truncate a UUID to first 8 characters.
 */
export function truncateId(id: string): string {
  return id.slice(0, 8);
}
