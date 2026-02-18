import { SearchInput } from '@hearth/ui';

interface CatalogHeaderProps {
  search: string;
  onSearchChange: (value: string) => void;
  username: string;
}

export function CatalogHeader({ search, onSearchChange, username }: CatalogHeaderProps) {
  return (
    <header className="sticky top-0 z-50 bg-[var(--color-surface-base)]/85 backdrop-blur-xl border-b border-[var(--color-border-subtle)] px-8">
      <div className="max-w-6xl mx-auto flex items-center gap-5 h-16">
        {/* Logo + brand */}
        <div className="flex items-center gap-3 shrink-0">
          <img src="/data/hearth.svg" alt="Hearth" className="w-8 h-8" />
          <h1 className="text-lg font-semibold tracking-tight whitespace-nowrap">
            <span className="text-[var(--color-ember)]">Hearth</span>
            <span className="text-[var(--color-text-secondary)] font-normal ml-1.5">Software Center</span>
          </h1>
        </div>

        {/* Search — centered, breathing room */}
        <div className="flex-1 max-w-md ml-auto">
          <SearchInput
            value={search}
            onChange={onSearchChange}
            placeholder="Search software..."
            autoComplete="off"
          />
        </div>

        {/* User badge */}
        {username && (
          <div className="flex items-center gap-2 px-3 py-1.5 bg-[var(--color-surface)] border border-[var(--color-border-subtle)] rounded-[var(--radius-sm)] text-[13px] text-[var(--color-text-secondary)] whitespace-nowrap shrink-0">
            <span className="w-2 h-2 rounded-full bg-[var(--color-success)]" />
            <span>{username}</span>
          </div>
        )}
      </div>
    </header>
  );
}
