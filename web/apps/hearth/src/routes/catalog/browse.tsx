import { useState, useMemo } from 'react';
import { SearchInput, PageHeader } from '@hearth/ui';
import { useCatalog, useRequests, getRequestStatus } from '../../api/catalog';
import { useAuth } from '../../useAuth';
import { FilterPills } from './components/FilterPills';
import { CatalogGrid } from './components/CatalogGrid';
import { SoftwareDetail } from './components/SoftwareDetail';
import type { CatalogEntry } from '../../api/types';

export function CatalogBrowsePage() {
  const params = new URLSearchParams(window.location.search);
  const { user } = useAuth();

  // Use URL params if provided, otherwise derive from auth context
  const machineId = params.get('machine_id') || '';
  const username = params.get('username')
    || user?.profile?.preferred_username as string
    || '';

  const [search, setSearch] = useState('');
  const [activeCategory, setActiveCategory] = useState('all');
  const [selectedEntry, setSelectedEntry] = useState<CatalogEntry | null>(null);

  const { data: catalog, isLoading: catalogLoading } = useCatalog();
  const { data: requests } = useRequests();

  // Extract sorted unique categories
  const categories = useMemo(() => {
    if (!catalog) return [];
    const cats = new Set<string>();
    for (const entry of catalog) {
      if (entry.category) cats.add(entry.category);
    }
    return [...cats].sort();
  }, [catalog]);

  // Filter entries by category + search
  const filtered = useMemo(() => {
    if (!catalog) return undefined;

    return catalog.filter((entry) => {
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
  }, [catalog, activeCategory, search]);

  const selectedStatus = selectedEntry
    ? getRequestStatus(requests, selectedEntry.id, machineId, username)
    : null;

  return (
    <div>
      <PageHeader
        title="Software Catalog"
        description="Browse and request software for your workstation"
      />

      <div className="max-w-sm">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder="Search software..."
          autoComplete="off"
        />
      </div>

      <FilterPills
        categories={categories}
        active={activeCategory}
        onSelect={setActiveCategory}
      />
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
    </div>
  );
}
