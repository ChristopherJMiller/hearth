import { useState } from 'react';
import {
  PageContainer,
  PageHeader,
  Card,
  Button,
  TextInput,
  KeyValueEditor,
  StatusChip,
  Callout,
  SkeletonCard,
} from '@hearth/ui';
import { useMyConfig, useUpdateMyConfig } from '../api/me';
import type { UpdateMyConfigRequest, UserConfig } from '../api/types';
import { formatDateTime } from '../lib/time';
import { LuGitBranch, LuTerminal, LuVariable, LuSave, LuUser } from 'react-icons/lu';

export function SettingsPage() {
  const { data: config, isLoading } = useMyConfig();

  if (isLoading || !config) {
    return (
      <PageContainer size="narrow">
        <PageHeader title="Settings" description="Loading your environment configuration…" />
        <SkeletonCard />
      </PageContainer>
    );
  }

  // The inner form is remounted whenever the config record changes so its
  // `useState` initializers re-run against the fresh data. This is simpler
  // than imperatively syncing server state into state inside the parent.
  return <SettingsForm key={config.id} config={config} />;
}

interface FormState {
  gitName: string;
  gitEmail: string;
  editor: string;
  aliases: Record<string, string>;
  sessionVars: Record<string, string>;
}

function initialFormState(config: UserConfig): FormState {
  const ovr = (config.overrides ?? {}) as Record<string, unknown>;
  const git = (ovr.git ?? {}) as Record<string, string>;
  return {
    gitName: git.user_name ?? '',
    gitEmail: git.user_email ?? '',
    editor: (ovr.editor as string) ?? '',
    aliases: (ovr.shell_aliases as Record<string, string>) ?? {},
    sessionVars: (ovr.session_variables as Record<string, string>) ?? {},
  };
}

function SettingsForm({ config }: { config: UserConfig }) {
  const updateConfig = useUpdateMyConfig();
  const [form, setForm] = useState<FormState>(() => initialFormState(config));
  const [saved, setSaved] = useState(false);

  const patch = <K extends keyof FormState>(key: K, value: FormState[K]) =>
    setForm((prev) => ({ ...prev, [key]: value }));

  const handleSave = () => {
    const body: UpdateMyConfigRequest = {
      git_user_name: form.gitName,
      git_user_email: form.gitEmail,
      editor: form.editor,
      shell_aliases: form.aliases,
      session_variables: form.sessionVars,
    };
    updateConfig.mutate(body, {
      onSuccess: () => {
        setSaved(true);
        setTimeout(() => setSaved(false), 3000);
      },
    });
  };

  return (
    <PageContainer size="narrow">
      <PageHeader
        eyebrow="Personal"
        title="Environment settings"
        description="Customize your desktop environment. Changes trigger a rebuild of your per-user closure."
      />

      <Card className="mb-card-gap">
        <div className="flex items-center gap-4 flex-wrap">
          <div className="w-12 h-12 rounded-md flex items-center justify-center bg-ember-faint text-ember">
            <LuUser size={20} />
          </div>
          <div className="flex flex-col gap-0.5">
            <span className="font-semibold text-text-primary capitalize text-base">
              {config.base_role} role
            </span>
            <span className="text-text-tertiary text-xs">
              Updated {formatDateTime(config.updated_at)}
            </span>
          </div>
          <div className="ml-auto">
            <StatusChip status={config.build_status} />
          </div>
        </div>
        {config.build_error && (
          <div className="mt-4">
            <Callout variant="danger" title="Build failed">
              {config.build_error}
            </Callout>
          </div>
        )}
      </Card>

      <div className="flex flex-col gap-card-gap">
        <Card>
          <div className="flex items-center gap-2 mb-5">
            <LuGitBranch size={16} className="text-text-tertiary" />
            <h3 className="font-semibold text-text-primary text-lg">Git</h3>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <TextInput
              label="Name"
              value={form.gitName}
              onChange={(v) => patch('gitName', v)}
              placeholder="Your Name"
            />
            <TextInput
              label="Email"
              value={form.gitEmail}
              onChange={(v) => patch('gitEmail', v)}
              placeholder="you@example.com"
              type="email"
            />
          </div>
        </Card>

        <Card>
          <div className="flex items-center gap-2 mb-5">
            <LuTerminal size={16} className="text-text-tertiary" />
            <h3 className="font-semibold text-text-primary text-lg">Editor</h3>
          </div>
          <TextInput
            label="Default editor ($EDITOR / $VISUAL)"
            value={form.editor}
            onChange={(v) => patch('editor', v)}
            placeholder="nano, vim, code…"
          />
        </Card>

        <Card>
          <div className="flex items-center gap-2 mb-5">
            <LuTerminal size={16} className="text-text-tertiary" />
            <h3 className="font-semibold text-text-primary text-lg">Shell aliases</h3>
          </div>
          <KeyValueEditor
            value={form.aliases}
            onChange={(v) => patch('aliases', v)}
            keyLabel="Alias"
            valueLabel="Command"
            keyPlaceholder="ll"
            valuePlaceholder="ls -lah"
            monoValues
          />
        </Card>

        <Card>
          <div className="flex items-center gap-2 mb-5">
            <LuVariable size={16} className="text-text-tertiary" />
            <h3 className="font-semibold text-text-primary text-lg">Session variables</h3>
          </div>
          <KeyValueEditor
            value={form.sessionVars}
            onChange={(v) => patch('sessionVars', v)}
            keyLabel="Variable"
            valueLabel="Value"
            keyPlaceholder="EDITOR"
            valuePlaceholder="nvim"
            monoValues
          />
        </Card>

        {updateConfig.isError && (
          <Callout variant="danger" title="Failed to save settings">
            {updateConfig.error?.message ?? 'Unknown error'}
          </Callout>
        )}
        {saved && (
          <Callout variant="success" title="Saved">
            A rebuild of your closure has been queued.
          </Callout>
        )}

        <div className="flex justify-end">
          <Button
            variant="primary"
            size="lg"
            onClick={handleSave}
            loading={updateConfig.isPending}
            leadingIcon={<LuSave size={15} />}
          >
            Save settings
          </Button>
        </div>
      </div>
    </PageContainer>
  );
}
