import { useState, useMemo } from 'react';
import {
  PageContainer,
  PageHeader,
  Card,
  SearchInput,
  Avatar,
  Tooltip,
  Callout,
  SkeletonCard,
  EmptyState,
} from '@hearth/ui';
import { LuMail, LuMessageSquare, LuCloud, LuClock, LuUsers } from 'react-icons/lu';
import { useDirectory } from '../api/directory';
import { formatRelativeTime } from '../lib/time';
import type { DirectoryPerson } from '../api/types';

function PersonCard({ person }: { person: DirectoryPerson }) {
  const displayName = person.display_name ?? person.username;

  return (
    <Card className="h-full">
      <div className="flex flex-col gap-4">
        <div className="flex items-start gap-3">
          <Avatar name={displayName} size="md" />
          <div className="min-w-0 flex-1">
            <h3
              className="font-semibold text-[var(--color-text-primary)] truncate text-base"
             
            >
              {displayName}
            </h3>
            <p className="text-[var(--color-text-tertiary)] truncate text-xs">
              @{person.username}
            </p>
          </div>
          {person.last_seen && (
            <Tooltip content={`Last seen ${formatRelativeTime(person.last_seen)}`}>
              <span className="w-2 h-2 rounded-full bg-[var(--color-success)] mt-2 shrink-0" />
            </Tooltip>
          )}
        </div>

        {person.groups.length > 0 && (
          <div className="flex flex-wrap gap-1.5">
            {person.groups.map((group) => (
              <span
                key={group}
                className="font-mono px-2 py-0.5 rounded-[6px] bg-[var(--color-surface-sunken)] text-[var(--color-text-secondary)] border border-[var(--color-border-subtle)] text-2xs"
               
              >
                {group.replace(/^hearth-/, '')}
              </span>
            ))}
          </div>
        )}

        <div className="flex items-center gap-2 pt-3 border-t border-[var(--color-border-subtle)]">
          {person.email && (
            <Tooltip content={person.email}>
              <a
                href={`mailto:${person.email}`}
                className="w-8 h-8 flex items-center justify-center rounded-[var(--radius-sm)] text-[var(--color-text-tertiary)] hover:text-[var(--color-ember)] hover:bg-[var(--color-ember-faint)] transition-colors"
              >
                <LuMail size={15} />
              </a>
            </Tooltip>
          )}
          {person.matrix_id && (
            <Tooltip content={person.matrix_id}>
              <a
                href={`https://matrix.to/#/${person.matrix_id}`}
                target="_blank"
                rel="noopener noreferrer"
                className="w-8 h-8 flex items-center justify-center rounded-[var(--radius-sm)] text-[var(--color-text-tertiary)] hover:text-[var(--color-ember)] hover:bg-[var(--color-ember-faint)] transition-colors"
              >
                <LuMessageSquare size={15} />
              </a>
            </Tooltip>
          )}
          {person.nextcloud_url && (
            <Tooltip content="Nextcloud profile">
              <a
                href={person.nextcloud_url}
                target="_blank"
                rel="noopener noreferrer"
                className="w-8 h-8 flex items-center justify-center rounded-[var(--radius-sm)] text-[var(--color-text-tertiary)] hover:text-[var(--color-ember)] hover:bg-[var(--color-ember-faint)] transition-colors"
              >
                <LuCloud size={15} />
              </a>
            </Tooltip>
          )}
          {person.last_seen && (
            <span
              className="ml-auto flex items-center gap-1 text-[var(--color-text-tertiary)] text-2xs"
             
            >
              <LuClock size={11} />
              {formatRelativeTime(person.last_seen)}
            </span>
          )}
        </div>
      </div>
    </Card>
  );
}

export function DirectoryPage() {
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
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Identity & access"
        title="Directory"
        description={
          people
            ? `${people.length} ${people.length === 1 ? 'person' : 'people'} across the org`
            : 'Everyone in your organization'
        }
      />

      <div className="max-w-md mb-[var(--spacing-section)]">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder="Search by name, username, email, or group…"
        />
      </div>

      {isError ? (
        <Callout variant="danger" title="Could not load directory" />
      ) : isLoading ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-[var(--spacing-card-gap)]">
          <SkeletonCard />
          <SkeletonCard />
          <SkeletonCard />
        </div>
      ) : filtered.length === 0 ? (
        <EmptyState
          icon={<LuUsers size={28} />}
          title={search ? 'No matches' : 'Empty directory'}
          description={search ? 'Try a different search term.' : 'People will appear here once they sign in.'}
        />
      ) : (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-[var(--spacing-card-gap)]">
          {filtered.map((person) => (
            <PersonCard key={person.username} person={person} />
          ))}
        </div>
      )}
    </PageContainer>
  );
}
