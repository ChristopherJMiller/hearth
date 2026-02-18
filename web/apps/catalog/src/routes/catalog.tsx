import { useState, useMemo } from 'react';
import { useCatalog, useRequests, getRequestStatus } from '../api';
import { CatalogHeader } from '../components/CatalogHeader';
import { FilterPills } from '../components/FilterPills';
import { CatalogGrid } from '../components/CatalogGrid';
import { SoftwareDetail } from '../components/SoftwareDetail';
import type { CatalogEntry } from '../types';

export function CatalogPage() {
  const params = new URLSearchParams(window.location.search);
  const machineId = params.get('machine_id') || '';
  const username = params.get('username') || '';

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
    <>
      <CatalogHeader
        search={search}
        onSearchChange={setSearch}
        username={username}
      />
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
    </>
  );
}
