import { useState, useMemo } from 'react';
import { type ColumnDef } from '@tanstack/react-table';
import {
  PageContainer,
  PageHeader,
  DataTable,
  SearchInput,
  Badge,
  StatusChip,
  SkeletonTable,
  Callout,
} from '@hearth/ui';
import type { BadgeVariant } from '@hearth/ui';
import { useCatalog } from '../../api/catalog';
import type { CatalogEntry } from '../../api/types';

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
      <div className="flex flex-col gap-0.5 min-w-0">
        <span
          className="font-semibold text-[var(--color-text-primary)] text-sm"
         
        >
          {row.original.name}
        </span>
        {row.original.description && (
          <span
            className="text-[var(--color-text-tertiary)] truncate max-w-[420px] text-xs"
           
          >
            {row.original.description}
          </span>
        )}
      </div>
    ),
  },
  {
    accessorKey: 'category',
    header: 'Category',
    cell: ({ row }) => (
      <span className="text-[var(--color-text-secondary)] text-sm">
        {row.original.category ?? '—'}
      </span>
    ),
  },
  {
    accessorKey: 'install_method',
    header: 'Method',
    cell: ({ row }) => {
      const method = row.original.install_method;
      const variant = installMethodBadge[method];
      const label = installMethodLabel[method] ?? method;
      return variant ? <Badge variant={variant}>{label}</Badge> : <span>{label}</span>;
    },
  },
  {
    accessorKey: 'approval_required',
    header: 'Approval',
    cell: ({ row }) =>
      row.original.approval_required ? (
        <StatusChip status="warning" tone="warning" label="Required" withDot={false} />
      ) : (
        <StatusChip status="success" tone="success" label="Auto" withDot={false} />
      ),
  },
  {
    id: 'auto_approve_roles',
    header: 'Auto-approve roles',
    enableSorting: false,
    cell: ({ row }) => {
      const roles = row.original.auto_approve_roles;
      if (roles.length === 0)
        return <span className="text-[var(--color-text-tertiary)] text-xs">—</span>;
      return (
        <div className="flex flex-wrap gap-1.5">
          {roles.map((role) => (
            <span
              key={role}
              className="font-mono px-2 py-0.5 rounded-[6px] bg-[var(--color-surface-sunken)] text-[var(--color-text-secondary)] border border-[var(--color-border-subtle)] text-2xs"
             
            >
              {role}
            </span>
          ))}
        </div>
      );
    },
  },
];

export function CatalogManagePage() {
  const { data: catalog, isLoading, isError } = useCatalog();
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
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Software"
        title="Manage catalog"
        description="The software your users can request. Approval rules, auto-approve by role, install method per entry."
      />

      <div className="max-w-md mb-4">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder="Search by name, category, or install method…"
        />
      </div>

      {isError ? (
        <Callout variant="danger" title="Could not load catalog" />
      ) : isLoading ? (
        <SkeletonTable rows={6} cols={5} />
      ) : (
        <DataTable
          data={filtered}
          columns={columns}
          emptyMessage="No catalog entries found"
          density="comfortable"
          pageSize={25}
        />
      )}
    </PageContainer>
  );
}
