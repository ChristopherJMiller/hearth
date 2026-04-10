import { Outlet } from '@tanstack/react-router';
import { AppShell } from '../components/shell/AppShell';

export function RootLayout() {
  return (
    <AppShell>
      <Outlet />
    </AppShell>
  );
}
