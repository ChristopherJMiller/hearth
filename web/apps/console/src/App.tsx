import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';
import { router } from './router';
import { AuthGuard } from './AuthGuard';
import { CallbackPage } from './routes/callback';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 15_000, retry: 1 },
  },
});

export function App() {
  // Handle OIDC callback before anything else — this path must be reachable
  // without authentication since it completes the login flow.
  if (window.location.pathname === '/console/callback') {
    return <CallbackPage />;
  }

  return (
    <QueryClientProvider client={queryClient}>
      <AuthGuard>
        <RouterProvider router={router} />
      </AuthGuard>
    </QueryClientProvider>
  );
}
