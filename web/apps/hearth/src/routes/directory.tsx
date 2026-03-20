import { useState, useMemo } from 'react';
import { PageHeader, Card, SearchInput } from '@hearth/ui';
import { LuMail, LuMessageSquare, LuCloud, LuClock } from 'react-icons/lu';
import { useDirectory } from '../api/directory';
import { formatRelativeTime } from '../lib/time';
import type { DirectoryPerson } from '../api/types';

function Initials({ name, username }: { name: string | null; username: string }) {
  const text = name ?? username;
  const initials = text
    .split(/\s+/)
    .slice(0, 2)
    .map((w) => w[0]?.toUpperCase() ?? '')
    .join('');

  return (
    <div className="w-10 h-10 rounded-full bg-[var(--color-ember)] flex items-center justify-center text-white text-sm font-semibold shrink-0">
      {initials || '?'}
    </div>
  );
}

function PersonCard({ person }: { person: DirectoryPerson }) {
  const displayName = person.display_name ?? person.username;

  return (
    <Card>
      <div className="p-5 space-y-3">
        <div className="flex items-start gap-3">
          <Initials name={person.display_name} username={person.username} />
          <div className="min-w-0 flex-1">
            <h3 className="text-sm font-semibold text-[var(--color-text-primary)] truncate">
              {displayName}
            </h3>
            <p className="text-xs text-[var(--color-text-secondary)] truncate">
              @{person.username}
            </p>
          </div>
        </div>

        {person.groups.length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {person.groups.map((group) => (
              <span
                key={group}
                className="inline-flex items-center text-[11px] font-medium px-2 py-0.5 rounded-md bg-[var(--color-surface-raised)] text-[var(--color-text-secondary)]"
              >
                {group.replace(/^hearth-/, '')}
              </span>
            ))}
          </div>
        )}

        <div className="flex items-center gap-3 text-[var(--color-text-muted)]">
          {person.email && (
            <a
              href={`mailto:${person.email}`}
              title={person.email}
              className="hover:text-[var(--color-ember)] transition-colors"
            >
              <LuMail size={15} />
            </a>
          )}
          {person.matrix_id && (
            <a
              href={`https://matrix.to/#/${person.matrix_id}`}
              target="_blank"
              rel="noopener noreferrer"
              title={person.matrix_id}
              className="hover:text-[var(--color-ember)] transition-colors"
            >
              <LuMessageSquare size={15} />
            </a>
          )}
          {person.nextcloud_url && (
            <a
              href={person.nextcloud_url}
              target="_blank"
              rel="noopener noreferrer"
              title="Nextcloud profile"
              className="hover:text-[var(--color-ember)] transition-colors"
            >
              <LuCloud size={15} />
            </a>
          )}
          {person.last_seen && (
            <span className="ml-auto flex items-center gap-1 text-xs text-[var(--color-text-muted)]" title={`Last seen: ${person.last_seen}`}>
              <LuClock size={12} />
              {formatRelativeTime(person.last_seen)}
            </span>
          )}
        </div>
      </div>
    </Card>
  );
}

export function DirectoryPage() {
  const { data: people, isLoading, error } = useDirectory();
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

  if (isLoading) {
    return (
      <div className="p-6">
        <PageHeader title="People" description="Loading directory..." />
      </div>
    );
  }

  if (error) {
    return (
      <div className="p-6">
        <PageHeader title="People" description="Failed to load directory." />
        <p className="text-sm text-[var(--color-ember)] mt-4">{error.message}</p>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      <PageHeader
        title="People"
        description={`${people?.length ?? 0} people in your organization`}
      />

      <div className="max-w-sm">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder="Search by name, username, email, or group..."
        />
      </div>

      {filtered.length === 0 ? (
        <p className="text-sm text-[var(--color-text-muted)]">
          {search ? 'No people match your search.' : 'No people in the directory yet.'}
        </p>
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
          {filtered.map((person) => (
            <PersonCard key={person.username} person={person} />
          ))}
        </div>
      )}
    </div>
  );
}
