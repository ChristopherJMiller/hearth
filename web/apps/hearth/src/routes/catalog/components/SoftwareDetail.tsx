import * as Dialog from '@radix-ui/react-dialog';
import { Badge, Button, StatusChip } from '@hearth/ui';
import type { BadgeVariant } from '@hearth/ui';
import { useRequestSoftware } from '../../../api/catalog';
import type { CatalogEntry, InstallMethod, SoftwareRequestStatus } from '../../../api/types';

interface SoftwareDetailProps {
  entry: CatalogEntry | null;
  status: SoftwareRequestStatus | null;
  machineId: string;
  username: string;
  onClose: () => void;
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
  const cat = (entry.category || '').toLowerCase();
  for (const [key, icon] of Object.entries(categoryIcons)) {
    if (cat.includes(key)) return icon;
  }
  return '\u{1F4E6}';
}

export function SoftwareDetail({ entry, status, machineId, username, onClose }: SoftwareDetailProps) {
  const mutation = useRequestSoftware();
  const hasCredentials = Boolean(machineId && username);

  if (!entry) return null;

  const handleRequest = () => {
    mutation.mutate({ catalogEntryId: entry.id, machineId, username });
  };

  const iconText = entry.icon_url ? '' : getIcon(entry);

  return (
    <Dialog.Root open={!!entry} onOpenChange={(open) => !open && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-[999] bg-black/60 backdrop-blur-sm animate-[fade-in_0.2s_ease_both]" />
        <Dialog.Content className="fixed z-[1000] top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-full max-w-lg bg-surface border border-border rounded-lg shadow-overlay animate-[fade-in-up_0.3s_ease_both] focus:outline-none">
          {/* Header area with colored accent stripe */}
          <div className="relative px-6 pt-6 pb-4">
            {/* Subtle ember glow at top */}
            <div className="absolute top-0 left-0 right-0 h-[2px] bg-gradient-to-r from-transparent via-ember to-transparent opacity-40" />

            <div className="flex items-start gap-4">
              {/* Large icon */}
              <div className="w-16 h-16 rounded-md bg-surface-raised border border-border-subtle flex items-center justify-center shrink-0 text-[28px]">
                {entry.icon_url ? (
                  <img src={entry.icon_url} alt="" className="w-11 h-11 rounded-lg object-cover" />
                ) : (
                  iconText
                )}
              </div>

              <div className="flex-1 min-w-0">
                <Dialog.Title className="text-xl font-bold tracking-tight text-text-primary leading-tight">
                  {entry.name}
                </Dialog.Title>
                {entry.category && (
                  <p className="text-sm text-text-tertiary mt-1">
                    {entry.category}
                  </p>
                )}
                <div className="mt-2">
                  <Badge variant={methodToBadgeVariant[entry.install_method]}>
                    {methodLabels[entry.install_method]}
                  </Badge>
                </div>
              </div>

              {/* Close */}
              <Dialog.Close className="text-text-tertiary hover:text-text-primary transition-colors p-1 -mt-1 -mr-1 rounded-md hover:bg-surface-raised">
                <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
                  <line x1="18" y1="6" x2="6" y2="18" />
                  <line x1="6" y1="6" x2="18" y2="18" />
                </svg>
              </Dialog.Close>
            </div>
          </div>

          {/* Body */}
          <div className="px-6 pb-2">
            {entry.description && (
              <Dialog.Description className="text-sm text-text-secondary leading-relaxed">
                {entry.description}
              </Dialog.Description>
            )}

            {/* Metadata grid */}
            <div className="mt-4 grid grid-cols-2 gap-3">
              {entry.flatpak_ref && (
                <div className="bg-surface-raised rounded-sm px-3 py-2">
                  <div className="text-2xs font-mono uppercase tracking-wider text-text-tertiary mb-0.5">
                    Flatpak Ref
                  </div>
                  <div className="text-sm font-mono text-text-primary truncate">
                    {entry.flatpak_ref}
                  </div>
                </div>
              )}
              {entry.nix_attr && (
                <div className="bg-surface-raised rounded-sm px-3 py-2">
                  <div className="text-2xs font-mono uppercase tracking-wider text-text-tertiary mb-0.5">
                    Nix Attribute
                  </div>
                  <div className="text-sm font-mono text-text-primary truncate">
                    {entry.nix_attr}
                  </div>
                </div>
              )}
              <div className="bg-surface-raised rounded-sm px-3 py-2">
                <div className="text-2xs font-mono uppercase tracking-wider text-text-tertiary mb-0.5">
                  Approval
                </div>
                <div className="text-sm text-text-primary">
                  {entry.approval_required ? 'Required' : 'Auto-approved'}
                </div>
              </div>
              {entry.auto_approve_roles.length > 0 && (
                <div className="bg-surface-raised rounded-sm px-3 py-2">
                  <div className="text-2xs font-mono uppercase tracking-wider text-text-tertiary mb-0.5">
                    Auto-approve Roles
                  </div>
                  <div className="text-sm font-mono text-text-primary">
                    {entry.auto_approve_roles.join(', ')}
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* Footer actions */}
          <div className="px-6 py-4 mt-2 border-t border-border-subtle flex items-center justify-between gap-3">
            {status ? (
              <StatusChip status={status} />
            ) : (
              <span />
            )}
            <div className="flex items-center gap-2">
              <Dialog.Close asChild>
                <Button variant="ghost">Close</Button>
              </Dialog.Close>
              {!status && hasCredentials && (
                <Button
                  variant="primary"
                  onClick={handleRequest}
                  disabled={mutation.isPending}
                >
                  {mutation.isPending ? 'Requesting\u2026' : 'Request Install'}
                </Button>
              )}
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
