import { useState } from 'react';
import { useRouter, useParams } from '@tanstack/react-router';
import {
  PageContainer,
  PageHeader,
  Card,
  Avatar,
  StatusChip,
  Select,
  KeyValueEditor,
  Button,
  Callout,
  SkeletonCard,
  DescriptionList,
} from '@hearth/ui';
import { LuArrowLeft, LuHammer, LuSave, LuUser } from 'react-icons/lu';
import { useUserConfig, useUpdateUserConfig, useRebuildUserConfig } from '../../api/people';
import { useDirectory } from '../../api/directory';
import type { UserConfig, DirectoryPerson } from '../../api/types';
import { formatDateTime } from '../../lib/time';

const roleOptions = [
  { value: 'default', label: 'Default' },
  { value: 'developer', label: 'Developer' },
  { value: 'designer', label: 'Designer' },
  { value: 'admin', label: 'Admin' },
];

export function PersonDetailPage() {
  const router = useRouter();
  const { username } = useParams({ strict: false }) as { username: string };
  const config = useUserConfig(username);
  const directory = useDirectory();
  const person = directory.data?.find((p) => p.username === username);
  const displayName = person?.display_name ?? username;

  if (config.isError) {
    return (
      <PageContainer size="default">
        <Callout variant="danger" title="No config found">
          This user does not yet have an environment configuration. Updating below will create one.
        </Callout>
      </PageContainer>
    );
  }

  if (config.isLoading || !config.data) {
    return (
      <PageContainer size="default">
        <PageHeader title="Loading…" />
        <SkeletonCard />
      </PageContainer>
    );
  }

  // Remount-on-change pattern: when the server record's `id` changes, React
  // discards the form state and the child re-initializes from the fresh data.
  return (
    <PersonDetailForm
      key={config.data.id}
      username={username}
      displayName={displayName}
      person={person}
      config={config.data}
      onBack={() => router.navigate({ to: '/people' })}
    />
  );
}

interface FormState {
  baseRole: string;
  overrides: Record<string, string>;
}

function initialFormState(config: UserConfig): FormState {
  const flat: Record<string, string> = {};
  for (const [k, v] of Object.entries(config.overrides ?? {})) {
    flat[k] = typeof v === 'string' ? v : JSON.stringify(v);
  }
  return { baseRole: config.base_role, overrides: flat };
}

function PersonDetailForm({
  username,
  displayName,
  person,
  config,
  onBack,
}: {
  username: string;
  displayName: string;
  person: DirectoryPerson | undefined;
  config: UserConfig;
  onBack: () => void;
}) {
  const update = useUpdateUserConfig(username);
  const rebuild = useRebuildUserConfig(username);
  const [form, setForm] = useState<FormState>(() => initialFormState(config));

  const handleSave = () => {
    // Accept user-entered JSON where valid (numbers, bools, arrays) and fall
    // back to raw strings for everything else. The backend round-trips
    // `overrides` as a generic `serde_json::Value` so either shape is fine.
    const parsed: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(form.overrides)) {
      try {
        parsed[k] = JSON.parse(v);
      } catch {
        parsed[k] = v;
      }
    }
    update.mutate({ base_role: form.baseRole, overrides: parsed });
  };

  return (
    <PageContainer size="default">
      <PageHeader
        eyebrow="Identity & access"
        title={displayName}
        description={`@${username}${person?.email ? ` · ${person.email}` : ''}`}
        breadcrumbs={[
          { label: 'Identity & access' },
          { label: 'People', onClick: onBack },
          { label: username },
        ]}
        actions={
          <Button variant="ghost" leadingIcon={<LuArrowLeft size={14} />} onClick={onBack}>
            Back
          </Button>
        }
      />

      <div className="grid grid-cols-1 lg:grid-cols-[280px_1fr] gap-card-gap items-start">
        <Card>
          <div className="flex flex-col items-center gap-3 text-center">
            <Avatar name={displayName} size="lg" />
            <div className="flex flex-col gap-1">
              <h2 className="font-semibold text-text-primary text-lg">
                {displayName}
              </h2>
              <span className="text-text-tertiary text-xs">@{username}</span>
            </div>
            <StatusChip status={config.build_status} />
          </div>
          <div className="mt-5 pt-5 border-t border-border-subtle">
            <DescriptionList
              columns={1}
              items={[
                { label: 'Base role', value: <span className="capitalize">{config.base_role}</span> },
                { label: 'Created', value: formatDateTime(config.created_at) },
                { label: 'Updated', value: formatDateTime(config.updated_at) },
                {
                  label: 'Latest closure',
                  value: config.latest_closure ? (
                    <span className="font-mono break-all text-2xs">{config.latest_closure}</span>
                  ) : (
                    <span className="italic text-text-tertiary">none</span>
                  ),
                },
              ]}
            />
          </div>
        </Card>

        <div className="flex flex-col gap-card-gap">
          <Card>
            <div className="flex items-center gap-2 mb-5">
              <LuUser size={16} className="text-text-tertiary" />
              <h2 className="font-semibold text-text-primary text-lg">Base role</h2>
            </div>
            <Select
              options={roleOptions}
              value={form.baseRole}
              onChange={(baseRole) => setForm((p) => ({ ...p, baseRole }))}
              label="Assigned role"
            />
          </Card>

          <Card>
            <div className="mb-2">
              <h2 className="font-semibold text-text-primary text-lg">Overrides</h2>
              <p className="text-text-tertiary text-xs">
                Per-user environment overrides. Values are parsed as JSON when valid, otherwise stored as strings.
              </p>
            </div>
            <KeyValueEditor
              value={form.overrides}
              onChange={(overrides) => setForm((p) => ({ ...p, overrides }))}
              keyLabel="Key"
              valueLabel="Value (JSON)"
              keyPlaceholder="editor"
              valuePlaceholder='"nvim"'
              monoValues
            />
          </Card>

          {config.build_error && (
            <Callout variant="danger" title="Last build failed">
              <pre className="font-mono whitespace-pre-wrap text-xs">{config.build_error}</pre>
            </Callout>
          )}

          {update.isError && <Callout variant="danger" title="Could not save changes" />}
          {update.isSuccess && <Callout variant="success" title="Saved" />}

          <div className="flex items-center justify-end gap-2">
            <Button
              variant="subtle"
              leadingIcon={<LuHammer size={14} />}
              loading={rebuild.isPending}
              onClick={() => rebuild.mutate()}
            >
              Force rebuild
            </Button>
            <Button
              variant="primary"
              size="lg"
              leadingIcon={<LuSave size={15} />}
              loading={update.isPending}
              onClick={handleSave}
            >
              Save changes
            </Button>
          </div>
        </div>
      </div>
    </PageContainer>
  );
}
