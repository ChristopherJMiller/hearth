import { useState, useMemo } from 'react';
import { type ColumnDef } from '@tanstack/react-table';
import { PageHeader, DataTable, SearchInput, Badge } from '@hearth/ui';
import type { BadgeVariant } from '@hearth/ui';
import { useCatalog } from '../../api/catalog';
import type { CatalogEntry } from '../../api/types';
import { LuShield, LuShieldOff } from 'react-icons/lu';

const installMethodBadge: Record<string, BadgeVariant> = {
  flatpak: 'flatpak',
  nix_system: 'nix-system',
  nix_user: 'nix-user',
  home_manager: 'home-manager',
};

const installMethodLabel: Record<string, string> = {
  flatpak: 'Flatpak',
  nix_system: 'Nix System',
  nix_user: 'Nix User',
  home_manager: 'Home Manager',
};

const columns: ColumnDef<CatalogEntry, unknown>[] = [
  {
    accessorKey: 'name',
    header: 'Name',
    cell: ({ row }) => (
      <div>
        <p className="font-medium text-[var(--color-text-primary)]">{row.original.name}</p>
        {row.original.description && (
          <p className="text-xs text-[var(--color-text-tertiary)] mt-0.5 max-w-[300px] truncate">
            {row.original.description}
          </p>
        )}
      </div>
    ),
  },
  {
    accessorKey: 'category',
    header: 'Category',
    cell: ({ row }) => (
      <span className="text-sm text-[var(--color-text-secondary)]">
        {row.original.category ?? '—'}
      </span>
    ),
  },
  {
    accessorKey: 'install_method',
    header: 'Install Method',
    cell: ({ row }) => {
      const method = row.original.install_method;
      const variant = installMethodBadge[method];
      const label = installMethodLabel[method] ?? method;
      return variant ? (
        <Badge variant={variant}>{label}</Badge>
      ) : (
        <span className="text-xs text-[var(--color-text-secondary)]">{label}</span>
      );
    },
  },
  {
    accessorKey: 'approval_required',
    header: 'Approval Required',
    cell: ({ row }) => {
      const required = row.original.approval_required;
      return (
        <div className="flex items-center gap-1.5">
          {required ? (
            <>
              <LuShield size={14} className="text-[var(--color-warning)]" />
              <span className="text-xs text-[var(--color-warning)] font-medium">Required</span>
            </>
          ) : (
            <>
              <LuShieldOff size={14} className="text-[var(--color-text-tertiary)]" />
              <span className="text-xs text-[var(--color-text-tertiary)]">Auto-approve</span>
            </>
          )}
        </div>
      );
    },
  },
  {
    id: 'auto_approve_roles',
    header: 'Auto-Approve Roles',
    enableSorting: false,
    cell: ({ row }) => {
      const roles = row.original.auto_approve_roles;
      if (roles.length === 0) {
        return <span className="text-sm text-[var(--color-text-tertiary)]">—</span>;
      }
      return (
        <div className="flex flex-wrap gap-1">
          {roles.map((role) => (
            <span
              key={role}
              className="text-[11px] font-mono px-1.5 py-0.5 rounded bg-[var(--color-surface-raised)] text-[var(--color-text-secondary)] border border-[var(--color-border-subtle)]"
            >
              {role}
            </span>
          ))}
        </div>
      );
    },
  },
];

export function CatalogPage() {
  const { data: catalog, isLoading } = useCatalog();
  const [search, setSearch] = useState('');

  const filtered = useMemo(() => {
    if (!catalog) return [];
    if (!search) return catalog;
    const q = search.toLowerCase();
    return catalog.filter(
      (entry) =>
        entry.name.toLowerCase().includes(q) ||
        (entry.description ?? '').toLowerCase().includes(q) ||
        (entry.category ?? '').toLowerCase().includes(q) ||
        entry.install_method.toLowerCase().includes(q),
    );
  }, [catalog, search]);

  return (
    <div>
      <PageHeader
        title="Software Catalog"
        description="Browse and manage the fleet software catalog"
      />

      <div className="mb-4 max-w-sm">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder="Search by name, category, or install method..."
        />
      </div>

      {isLoading ? (
        <p className="text-sm text-[var(--color-text-tertiary)] py-12 text-center">
          Loading catalog...
        </p>
      ) : (
        <DataTable
          data={filtered}
          columns={columns}
          emptyMessage="No catalog entries found"
        />
      )}
    </div>
  );
}
