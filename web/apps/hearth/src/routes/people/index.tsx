import { useState, useMemo } from 'react';
import { useRouter } from '@tanstack/react-router';
import {
  PageContainer,
  PageHeader,
  SearchInput,
  Avatar,
  StatusChip,
  Callout,
  SkeletonCard,
  EmptyState,
} from '@hearth/ui';
import { LuContact } from 'react-icons/lu';
import { useDirectory } from '../../api/directory';
import type { DirectoryPerson } from '../../api/types';

function PersonRow({
  person,
  onClick,
}: {
  person: DirectoryPerson;
  onClick: () => void;
}) {
  const displayName = person.display_name ?? person.username;
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center gap-4 w-full text-left p-4 rounded-[var(--radius-sm)] bg-[var(--color-surface)] border border-[var(--color-border-subtle)] hover:border-[var(--color-border-accent)] hover:bg-[var(--color-surface-raised)] transition-colors cursor-pointer"
    >
      <Avatar name={displayName} size="md" />
      <div className="flex-1 min-w-0">
        <div
          className="font-semibold text-[var(--color-text-primary)] truncate text-sm"
         
        >
          {displayName}
        </div>
        <div
          className="text-[var(--color-text-tertiary)] truncate text-xs"
         
        >
          @{person.username}
          {person.email && ` · ${person.email}`}
        </div>
      </div>
      <div className="flex flex-wrap gap-1.5 max-w-[40%] justify-end">
        {person.groups.slice(0, 3).map((g) => (
          <StatusChip key={g} status="info" tone="neutral" label={g.replace(/^hearth-/, '')} withDot={false} size="sm" />
        ))}
      </div>
    </button>
  );
}

export function PeoplePage() {
  const router = useRouter();
  const { data: people, isLoading, isError } = useDirectory();
  const [search, setSearch] = useState('');

  const filtered = useMemo(() => {
    if (!people) return [];
    if (!search.trim()) return people;
    const q = search.toLowerCase();
    return people.filter(
      (p) =>
        p.username.toLowerCase().includes(q) ||
        (p.display_name?.toLowerCase().includes(q) ?? false) ||
        (p.email?.toLowerCase().includes(q) ?? false) ||
        p.groups.some((g) => g.toLowerCase().includes(q)),
    );
  }, [people, search]);

  return (
    <PageContainer size="default">
      <PageHeader
        eyebrow="Identity & access"
        title="People"
        description="Manage per-user environment configuration. Click anyone to set their base role, overrides, and trigger a closure rebuild."
      />

      <div className="max-w-md mb-[var(--spacing-section)]">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder="Search by name, username, email, or group…"
        />
      </div>

      {isError ? (
        <Callout variant="danger" title="Could not load people" />
      ) : isLoading ? (
        <div className="flex flex-col gap-3">
          <SkeletonCard />
          <SkeletonCard />
          <SkeletonCard />
        </div>
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={<LuContact size={28} />}
          title={search ? 'No matches' : 'No people'}
          description={search ? 'Try a different query.' : 'People will appear here as they sign in.'}
        />
      ) : (
        <div className="flex flex-col gap-2">
          {filtered.map((person) => (
            <PersonRow
              key={person.username}
              person={person}
              onClick={() =>
                router.navigate({
                  to: '/people/$username',
                  params: { username: person.username },
                })
              }
            />
          ))}
        </div>
      )}
    </PageContainer>
  );
}
