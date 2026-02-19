import { CatalogCard } from './CatalogCard';
import { getRequestStatus } from '../api';
import type { CatalogEntry, SoftwareRequest } from '../types';

interface CatalogGridProps {
  entries: CatalogEntry[] | undefined;
  requests: SoftwareRequest[] | undefined;
  machineId: string;
  username: string;
  isLoading: boolean;
  hasSearchQuery: boolean;
  onSelectEntry: (entry: CatalogEntry) => void;
}

function SkeletonCard({ index }: { index: number }) {
  return (
    <div
      className="bg-[var(--color-surface)] border border-[var(--color-border-subtle)] rounded-[var(--radius-md)] p-6 h-[200px] relative overflow-hidden animate-[fade-in_0.3s_ease_both]"
      style={{ animationDelay: `${index * 60}ms` }}
    >
      {/* Shimmer */}
      <div className="absolute inset-0 bg-gradient-to-r from-transparent via-white/[0.03] to-transparent animate-[shimmer_1.5s_ease-in-out_infinite]" />

      {/* Skeleton content shapes */}
      <div className="flex items-start gap-3.5">
        <div className="w-12 h-12 rounded-[var(--radius-sm)] bg-[var(--color-surface-raised)]" />
        <div className="flex-1 space-y-2 pt-1">
          <div className="h-4 w-3/5 rounded bg-[var(--color-surface-raised)]" />
          <div className="h-3 w-2/5 rounded bg-[var(--color-surface-raised)]" />
        </div>
      </div>
      <div className="mt-4 space-y-2">
        <div className="h-3 w-full rounded bg-[var(--color-surface-raised)]" />
        <div className="h-3 w-4/5 rounded bg-[var(--color-surface-raised)]" />
      </div>
    </div>
  );
}

function EmptyState({ hasSearchQuery }: { hasSearchQuery: boolean }) {
  return (
    <div className="col-span-full text-center py-24 px-5 animate-[fade-in_0.4s_ease_both]">
      <div className="w-16 h-16 mx-auto mb-5 rounded-2xl bg-[var(--color-surface-raised)] border border-[var(--color-border-subtle)] flex items-center justify-center">
        <svg
          className="w-8 h-8 text-[var(--color-text-tertiary)]"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
        >
          {hasSearchQuery ? (
            <>
              <circle cx="11" cy="11" r="8" />
              <line x1="21" y1="21" x2="16.65" y2="16.65" />
            </>
          ) : (
            <>
              <path d="M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4" />
            </>
          )}
        </svg>
      </div>
      <h3 className="text-lg font-semibold text-[var(--color-text-primary)] mb-1">
        {hasSearchQuery ? 'No matches' : 'Catalog is empty'}
      </h3>
      <p className="text-sm text-[var(--color-text-tertiary)] max-w-xs mx-auto">
        {hasSearchQuery
          ? 'Try broadening your search or clearing the filters.'
          : 'Software will appear here once it\u2019s added to the catalog.'}
      </p>
    </div>
  );
}

export function CatalogGrid({
  entries,
  requests,
  machineId,
  username,
  isLoading,
  hasSearchQuery,
  onSelectEntry,
}: CatalogGridProps) {
  if (isLoading || !entries) {
    return (
      <div className="max-w-6xl mx-auto mt-6 mb-12 px-8 grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-5">
        {Array.from({ length: 6 }, (_, i) => (
          <SkeletonCard key={i} index={i} />
        ))}
      </div>
    );
  }

  if (entries.length === 0) {
    return (
      <div className="max-w-6xl mx-auto mt-6 mb-12 px-8 grid grid-cols-1">
        <EmptyState hasSearchQuery={hasSearchQuery} />
      </div>
    );
  }

  return (
    <div className="max-w-6xl mx-auto mt-6 mb-12 px-8 grid grid-cols-[repeat(auto-fill,minmax(300px,1fr))] gap-5">
      {entries.map((entry, i) => (
        <CatalogCard
          key={entry.id}
          entry={entry}
          status={getRequestStatus(requests, entry.id, machineId, username)}
          machineId={machineId}
          username={username}
          index={i}
          onSelect={onSelectEntry}
        />
      ))}
    </div>
  );
}
