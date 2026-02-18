import type { ReactNode } from 'react';
import { Card, Badge, Button, StatusChip } from '@hearth/ui';
import type { BadgeVariant } from '@hearth/ui';
import { useRequestSoftware } from '../api';
import type { CatalogEntry, InstallMethod, SoftwareRequestStatus } from '../types';

interface CatalogCardProps {
  entry: CatalogEntry;
  status: SoftwareRequestStatus | null;
  machineId: string;
  username: string;
  index: number;
  onSelect: (entry: CatalogEntry) => void;
}

const categoryIcons: Record<string, string> = {
  browser: '\u{1F310}',
  editor: '\u{270F}\uFE0F',
  development: '\u{1F4BB}',
  media: '\u{1F3A5}',
  communication: '\u{1F4AC}',
  graphics: '\u{1F3A8}',
  office: '\u{1F4C4}',
  utility: '\u{1F527}',
  game: '\u{1F3AE}',
  security: '\u{1F512}',
  system: '\u{2699}\uFE0F',
};

const methodLabels: Record<InstallMethod, string> = {
  flatpak: 'Flatpak',
  nix_system: 'Nix System',
  nix_user: 'Nix User',
  home_manager: 'Home Manager',
};

const methodToBadgeVariant: Record<InstallMethod, BadgeVariant> = {
  flatpak: 'flatpak',
  nix_system: 'nix-system',
  nix_user: 'nix-user',
  home_manager: 'home-manager',
};

function getIcon(entry: CatalogEntry): string {
  if (entry.icon_url) return '';
  const cat = (entry.category || '').toLowerCase();
  for (const [key, icon] of Object.entries(categoryIcons)) {
    if (cat.includes(key)) return icon;
  }
  return '\u{1F4E6}';
}

export function CatalogCard({ entry, status, machineId, username, index, onSelect }: CatalogCardProps) {
  const mutation = useRequestSoftware();
  const hasCredentials = Boolean(machineId && username);

  const handleRequest = (e: React.MouseEvent) => {
    e.stopPropagation();
    mutation.mutate({ catalogEntryId: entry.id, machineId, username });
  };

  const iconText = getIcon(entry);

  let action: ReactNode;
  if (!hasCredentials) {
    action = (
      <span className="text-xs text-[var(--color-text-tertiary)] italic">
        No credentials
      </span>
    );
  } else if (status) {
    action = <StatusChip status={status} />;
  } else {
    action = (
      <Button
        variant="primary"
        size="sm"
        onClick={handleRequest}
        disabled={mutation.isPending}
      >
        {mutation.isPending ? 'Requesting\u2026' : 'Request'}
      </Button>
    );
  }

  return (
    <Card
      animationDelay={index * 50}
      className="cursor-pointer group"
      onClick={() => onSelect(entry)}
    >
      <div className="flex flex-col gap-3 h-full">
        {/* Top: icon + name */}
        <div className="flex items-start gap-3.5">
          <div className="w-12 h-12 rounded-[var(--radius-sm)] bg-[var(--color-surface-raised)] border border-[var(--color-border-subtle)] flex items-center justify-center shrink-0 text-[22px] transition-colors duration-200 group-hover:border-[var(--color-border)]">
            {entry.icon_url ? (
              <img src={entry.icon_url} alt="" className="w-8 h-8 rounded-md object-cover" />
            ) : (
              iconText
            )}
          </div>
          <div className="flex-1 min-w-0">
            <div className="text-[15px] font-semibold tracking-tight leading-snug text-[var(--color-text-primary)]">
              {entry.name}
            </div>
            {entry.category && (
              <div className="text-xs text-[var(--color-text-tertiary)] mt-0.5">
                {entry.category}
              </div>
            )}
          </div>
        </div>

        {/* Description */}
        {entry.description && (
          <p className="text-sm text-[var(--color-text-secondary)] leading-relaxed flex-1 line-clamp-3">
            {entry.description}
          </p>
        )}

        {/* Footer: badge + action */}
        <div className="flex items-center justify-between gap-3 mt-auto pt-1">
          <Badge variant={methodToBadgeVariant[entry.install_method]}>
            {methodLabels[entry.install_method]}
          </Badge>
          {action}
        </div>
      </div>
    </Card>
  );
}
