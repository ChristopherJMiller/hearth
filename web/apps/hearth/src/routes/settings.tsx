import { useState, useRef } from 'react';
import { PageHeader, Card, Button } from '@hearth/ui';
import { useMyConfig, useUpdateMyConfig } from '../api/me';
import type { UpdateMyConfigRequest } from '../api/types';

interface KvEntry {
  id: number;
  key: string;
  value: string;
}

let nextKvId = 0;

function kvFromRecord(record: Record<string, string>): KvEntry[] {
  return Object.entries(record).map(([key, value]) => ({
    id: nextKvId++,
    key,
    value,
  }));
}

function kvToRecord(entries: KvEntry[]): Record<string, string> {
  const result: Record<string, string> = {};
  for (const entry of entries) {
    if (entry.key) result[entry.key] = entry.value;
  }
  return result;
}

function KeyValueEditor({
  label,
  value,
  onChange,
}: {
  label: string;
  value: KvEntry[];
  onChange: (v: KvEntry[]) => void;
}) {
  const addEntry = () => {
    onChange([...value, { id: nextKvId++, key: '', value: '' }]);
  };

  const removeEntry = (id: number) => {
    onChange(value.filter((e) => e.id !== id));
  };

  const updateEntry = (id: number, field: 'key' | 'value', newVal: string) => {
    onChange(
      value.map((e) => (e.id === id ? { ...e, [field]: newVal } : e)),
    );
  };

  return (
    <div className="space-y-2">
      {label && (
        <div className="flex items-center justify-between">
          <label className="text-sm font-medium text-[var(--color-text-secondary)]">
            {label}
          </label>
        </div>
      )}
      {value.length === 0 && (
        <p className="text-xs text-[var(--color-text-muted)] italic">None configured</p>
      )}
      {value.map((entry) => (
        <div key={entry.id} className="flex gap-2 items-center">
          <input
            type="text"
            value={entry.key}
            placeholder="key"
            onChange={(e) => updateEntry(entry.id, 'key', e.target.value)}
            className="flex-1 rounded bg-[var(--color-surface-base)] border border-[var(--color-border)] px-2 py-1 text-sm text-[var(--color-text-primary)]"
          />
          <span className="text-[var(--color-text-muted)]">=</span>
          <input
            type="text"
            value={entry.value}
            placeholder="value"
            onChange={(e) => updateEntry(entry.id, 'value', e.target.value)}
            className="flex-1 rounded bg-[var(--color-surface-base)] border border-[var(--color-border)] px-2 py-1 text-sm text-[var(--color-text-primary)]"
          />
          <button
            type="button"
            onClick={() => removeEntry(entry.id)}
            className="text-xs text-[var(--color-text-muted)] hover:text-[var(--color-ember)]"
          >
            Remove
          </button>
        </div>
      ))}
      <button
        type="button"
        onClick={addEntry}
        className="text-xs text-[var(--color-ember)] hover:underline"
      >
        + Add
      </button>
    </div>
  );
}

export function SettingsPage() {
  const { data: config, isLoading } = useMyConfig();
  const updateConfig = useUpdateMyConfig();
  const initializedRef = useRef<string | null>(null);

  const [gitName, setGitName] = useState('');
  const [gitEmail, setGitEmail] = useState('');
  const [editor, setEditor] = useState('');
  const [aliases, setAliases] = useState<KvEntry[]>([]);
  const [sessionVars, setSessionVars] = useState<KvEntry[]>([]);
  const [saved, setSaved] = useState(false);

  // Populate form from config only on initial load (not after saves).
  if (config && initializedRef.current !== config.id) {
    initializedRef.current = config.id;
    const ovr = config.overrides as Record<string, unknown>;
    const git = (ovr?.git ?? {}) as Record<string, string>;
    setGitName(git.user_name ?? '');
    setGitEmail(git.user_email ?? '');
    setEditor((ovr?.editor as string) ?? '');
    setAliases(kvFromRecord((ovr?.shell_aliases as Record<string, string>) ?? {}));
    setSessionVars(kvFromRecord((ovr?.session_variables as Record<string, string>) ?? {}));
  }

  const handleSave = () => {
    const body: UpdateMyConfigRequest = {
      git_user_name: gitName,
      git_user_email: gitEmail,
      editor: editor,
      shell_aliases: kvToRecord(aliases),
      session_variables: kvToRecord(sessionVars),
    };

    updateConfig.mutate(body, {
      onSuccess: () => {
        setSaved(true);
        setTimeout(() => setSaved(false), 3000);
      },
    });
  };

  if (isLoading) {
    return (
      <div className="p-6">
        <PageHeader title="Settings" description="Loading your environment configuration..." />
      </div>
    );
  }

  return (
    <div className="p-6 max-w-2xl space-y-6">
      <PageHeader
        title="Environment Settings"
        description="Customize your desktop environment. Changes trigger a rebuild of your per-user closure."
      />

      {config && (
        <div className="flex items-center gap-4 text-sm text-[var(--color-text-secondary)]">
          <span>Role: <strong className="text-[var(--color-text-primary)]">{config.base_role}</strong></span>
          <span className={`text-xs px-2 py-0.5 rounded ${
            config.build_status === 'built' ? 'bg-green-900/40 text-green-400' :
            config.build_status === 'failed' ? 'bg-red-900/40 text-red-400' :
            config.build_status === 'building' ? 'bg-yellow-900/40 text-yellow-400' :
            'bg-gray-700/40 text-gray-400'
          }`}>
            {config.build_status}
          </span>
          {config.build_error && (
            <span className="text-[var(--color-ember)]">{config.build_error}</span>
          )}
        </div>
      )}

      <Card>
        <div className="p-5 space-y-5">
          <h3 className="text-sm font-semibold text-[var(--color-text-primary)] uppercase tracking-wider">
            Git Configuration
          </h3>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm text-[var(--color-text-secondary)] mb-1">Name</label>
              <input
                type="text"
                value={gitName}
                onChange={(e) => setGitName(e.target.value)}
                placeholder="Your Name"
                className="w-full rounded bg-[var(--color-surface-base)] border border-[var(--color-border)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
              />
            </div>
            <div>
              <label className="block text-sm text-[var(--color-text-secondary)] mb-1">Email</label>
              <input
                type="email"
                value={gitEmail}
                onChange={(e) => setGitEmail(e.target.value)}
                placeholder="you@example.com"
                className="w-full rounded bg-[var(--color-surface-base)] border border-[var(--color-border)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
              />
            </div>
          </div>
        </div>
      </Card>

      <Card>
        <div className="p-5 space-y-5">
          <h3 className="text-sm font-semibold text-[var(--color-text-primary)] uppercase tracking-wider">
            Editor
          </h3>
          <div>
            <label className="block text-sm text-[var(--color-text-secondary)] mb-1">
              Default editor ($EDITOR / $VISUAL)
            </label>
            <input
              type="text"
              value={editor}
              onChange={(e) => setEditor(e.target.value)}
              placeholder="nano, vim, code, etc."
              className="w-full rounded bg-[var(--color-surface-base)] border border-[var(--color-border)] px-3 py-2 text-sm text-[var(--color-text-primary)]"
            />
          </div>
        </div>
      </Card>

      <Card>
        <div className="p-5 space-y-4">
          <h3 className="text-sm font-semibold text-[var(--color-text-primary)] uppercase tracking-wider">
            Shell Aliases
          </h3>
          <KeyValueEditor label="" value={aliases} onChange={setAliases} />
        </div>
      </Card>

      <Card>
        <div className="p-5 space-y-4">
          <h3 className="text-sm font-semibold text-[var(--color-text-primary)] uppercase tracking-wider">
            Session Variables
          </h3>
          <KeyValueEditor label="" value={sessionVars} onChange={setSessionVars} />
        </div>
      </Card>

      <div className="flex items-center gap-4">
        <Button
          onClick={handleSave}
          disabled={updateConfig.isPending}
        >
          {updateConfig.isPending ? 'Saving...' : 'Save Settings'}
        </Button>
        {saved && (
          <span className="text-sm text-green-400">Settings saved. A rebuild has been queued.</span>
        )}
        {updateConfig.isError && (
          <span className="text-sm text-[var(--color-ember)]">
            Failed to save: {updateConfig.error?.message}
          </span>
        )}
      </div>

      {config && (
        <p className="text-xs text-[var(--color-text-muted)]">
          Last updated: {new Date(config.updated_at).toLocaleString()}
        </p>
      )}
    </div>
  );
}
