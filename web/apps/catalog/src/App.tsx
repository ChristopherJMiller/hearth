import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { CatalogPage } from './routes/catalog';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 30_000, retry: 1 },
  },
});

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <CatalogPage />
    </QueryClientProvider>
  );
}
