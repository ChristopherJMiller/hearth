import { useState, useMemo } from 'react';
import { SearchInput, PageContainer, PageHeader, SegmentedControl } from '@hearth/ui';
import { useCatalog, useRequests, getRequestStatus } from '../../api/catalog';
import { useActor } from '../../hooks/useActor';
import { FilterPills } from './components/FilterPills';
import { CatalogGrid } from './components/CatalogGrid';
import { SoftwareDetail } from './components/SoftwareDetail';
import type { CatalogEntry } from '../../api/types';

type SortMode = 'name' | 'recent' | 'category';

export function CatalogBrowsePage() {
  const params = new URLSearchParams(window.location.search);
  const actor = useActor();

  const machineId = params.get('machine_id') || '';
  const username = params.get('username') || actor;

  const [search, setSearch] = useState('');
  const [activeCategory, setActiveCategory] = useState('all');
  const [sort, setSort] = useState<SortMode>('name');
  const [selectedEntry, setSelectedEntry] = useState<CatalogEntry | null>(null);

  const { data: catalog, isLoading: catalogLoading } = useCatalog();
  const { data: requests } = useRequests();

  const categories = useMemo(() => {
    if (!catalog) return [];
    const cats = new Set<string>();
    for (const entry of catalog) {
      if (entry.category) cats.add(entry.category);
    }
    return [...cats].sort();
  }, [catalog]);

  const filtered = useMemo(() => {
    if (!catalog) return undefined;
    const result = catalog.filter((entry) => {
      if (activeCategory !== 'all' && entry.category !== activeCategory) return false;
      if (search) {
        const q = search.toLowerCase();
        const name = (entry.name || '').toLowerCase();
        const desc = (entry.description || '').toLowerCase();
        const cat = (entry.category || '').toLowerCase();
        if (!name.includes(q) && !desc.includes(q) && !cat.includes(q)) return false;
      }
      return true;
    });
    if (sort === 'name') {
      result.sort((a, b) => (a.name ?? '').localeCompare(b.name ?? ''));
    } else if (sort === 'recent') {
      result.sort((a, b) => (b.created_at ?? '').localeCompare(a.created_at ?? ''));
    } else if (sort === 'category') {
      result.sort((a, b) => (a.category ?? '').localeCompare(b.category ?? ''));
    }
    return result;
  }, [catalog, activeCategory, search, sort]);

  const selectedStatus = selectedEntry
    ? getRequestStatus(requests, selectedEntry.id, machineId, username)
    : null;

  return (
    <PageContainer size="wide">
      <PageHeader
        eyebrow="Software"
        title="Catalog"
        description="Browse and request software for your workstation. Approved installs are deployed automatically."
      />

      <div className="flex items-center justify-between gap-3 flex-wrap mb-4">
        <div className="min-w-[280px] flex-1 max-w-md">
          <SearchInput
            value={search}
            onChange={setSearch}
            placeholder="Search software…"
            autoComplete="off"
          />
        </div>
        <SegmentedControl
          value={sort}
          onChange={setSort}
          size="sm"
          options={[
            { value: 'name', label: 'Name' },
            { value: 'recent', label: 'Recent' },
            { value: 'category', label: 'Category' },
          ]}
        />
      </div>

      <FilterPills categories={categories} active={activeCategory} onSelect={setActiveCategory} />

      <CatalogGrid
        entries={filtered}
        requests={requests}
        machineId={machineId}
        username={username}
        isLoading={catalogLoading}
        hasSearchQuery={search.length > 0}
        onSelectEntry={setSelectedEntry}
      />
      <SoftwareDetail
        entry={selectedEntry}
        status={selectedStatus}
        machineId={machineId}
        username={username}
        onClose={() => setSelectedEntry(null)}
      />
    </PageContainer>
  );
}
