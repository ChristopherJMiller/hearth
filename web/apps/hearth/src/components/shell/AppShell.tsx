import { type ReactNode } from 'react';
import { ShellProvider } from './ShellContext';
import { NavSidebar } from './NavSidebar';
import { TopBar } from './TopBar';
import { CommandPaletteHost } from './CommandPaletteHost';
import { NotificationCenter } from './NotificationCenter';

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <ShellProvider>
      <div className="flex h-screen bg-surface-base text-text-primary">
        <NavSidebar />
        <div className="flex flex-col flex-1 min-w-0">
          <TopBar />
          <main className="flex-1 overflow-y-auto px-page-x py-page-y">
            {children}
          </main>
        </div>
        <CommandPaletteHost />
        <NotificationCenter />
      </div>
    </ShellProvider>
  );
}
