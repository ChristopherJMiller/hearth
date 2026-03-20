-- Hearth Demo Environment: Seed Data
-- Idempotent — safe to run multiple times via ON CONFLICT DO NOTHING.
-- Run after migrations: psql $DATABASE_URL -f dev/seed-demo-data.sql

BEGIN;

-- Guard: skip audit_events seeding if data already exists
-- (audit_events has no unique constraint, so we check for a known seed event UUID)

-- ============================================================================
-- Machines (8 devices across the fleet)
-- ============================================================================

INSERT INTO machines (id, hostname, hardware_fingerprint, enrollment_status, role, tags,
                      current_closure, target_closure, rollback_closure,
                      last_heartbeat, serial_number, hardware_report,
                      headscale_ip, headscale_node_id,
                      created_at, updated_at)
VALUES
  -- Fleet VM (dev): active developer workstation
  ('00000000-0000-0000-0000-000000000001', 'hearth-fleet-vm',
   'dev-vm-fingerprint-001', 'active', 'developer', ARRAY['dev', 'vm'],
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   '/nix/store/prev789xyz-nixos-system-hearth-25.05',
   now() - interval '30 seconds', 'VM-DEV-001',
   '{"cpu": "QEMU Virtual CPU", "cores": 2, "ram_gb": 2, "disks": [{"name": "vda", "size_gb": 20}]}',
   '100.64.0.2', 'node-001',
   now() - interval '30 days', now()),

  -- Engineering workstation 1: active, has pending update
  ('00000000-0000-0000-0000-000000000002', 'eng-ws-01',
   'hw-fp-eng-ws-01', 'active', 'developer', ARRAY['engineering', 'floor-3'],
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   '/nix/store/newbuild789ab-nixos-system-hearth-25.05',
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   now() - interval '2 minutes', 'SN-ENG-001',
   '{"cpu": "AMD Ryzen 9 7950X", "cores": 16, "ram_gb": 64, "gpu": "NVIDIA RTX 4090", "disks": [{"name": "nvme0n1", "size_gb": 2000}]}',
   '100.64.0.3', 'node-002',
   now() - interval '60 days', now()),

  -- Engineering workstation 2: active
  ('00000000-0000-0000-0000-000000000003', 'eng-ws-02',
   'hw-fp-eng-ws-02', 'active', 'developer', ARRAY['engineering', 'floor-3'],
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   NULL,
   now() - interval '5 minutes', 'SN-ENG-002',
   '{"cpu": "AMD Ryzen 7 7800X3D", "cores": 8, "ram_gb": 32, "gpu": "NVIDIA RTX 4070", "disks": [{"name": "nvme0n1", "size_gb": 1000}]}',
   '100.64.0.4', 'node-003',
   now() - interval '45 days', now()),

  -- Design workstation: active
  ('00000000-0000-0000-0000-000000000004', 'design-ws-01',
   'hw-fp-design-ws-01', 'active', 'designer', ARRAY['design', 'floor-2'],
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   NULL,
   now() - interval '10 minutes', 'SN-DES-001',
   '{"cpu": "Apple M2 Pro (VM)", "cores": 12, "ram_gb": 32, "gpu": "Integrated", "disks": [{"name": "nvme0n1", "size_gb": 1000}]}',
   '100.64.0.5', 'node-004',
   now() - interval '90 days', now()),

  -- Executive laptop: enrolled, no heartbeat yet
  ('00000000-0000-0000-0000-000000000005', 'exec-laptop-01',
   'hw-fp-exec-laptop-01', 'enrolled', 'default', ARRAY['executive', 'mobile'],
   NULL,
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   NULL,
   NULL, 'SN-EXEC-001',
   '{"cpu": "Intel Core i7-1365U", "cores": 10, "ram_gb": 16, "disks": [{"name": "nvme0n1", "size_gb": 512}]}',
   NULL, NULL,
   now() - interval '3 days', now() - interval '3 days'),

  -- Conference kiosk: active, default role
  ('00000000-0000-0000-0000-000000000006', 'conference-kiosk-01',
   'hw-fp-kiosk-01', 'active', 'default', ARRAY['kiosk', 'lobby'],
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   NULL,
   now() - interval '1 minute', 'SN-KIOSK-001',
   '{"cpu": "Intel Celeron N5105", "cores": 4, "ram_gb": 8, "disks": [{"name": "sda", "size_gb": 128}]}',
   '100.64.0.6', 'node-005',
   now() - interval '180 days', now()),

  -- Staging build server: provisioning
  ('00000000-0000-0000-0000-000000000007', 'staging-build-01',
   'hw-fp-staging-build-01', 'provisioning', 'admin', ARRAY['infrastructure', 'staging'],
   NULL,
   '/nix/store/newbuild789ab-nixos-system-hearth-25.05',
   NULL,
   NULL, 'SN-STG-001',
   '{"cpu": "AMD EPYC 7763", "cores": 64, "ram_gb": 256, "disks": [{"name": "nvme0n1", "size_gb": 4000}, {"name": "nvme1n1", "size_gb": 4000}]}',
   NULL, NULL,
   now() - interval '1 day', now() - interval '1 day'),

  -- New hire pending enrollment
  ('00000000-0000-0000-0000-000000000008', 'new-hire-ws',
   'hw-fp-new-hire-ws', 'pending', NULL, ARRAY['onboarding'],
   NULL, NULL, NULL,
   NULL, 'SN-NEW-001',
   '{"cpu": "AMD Ryzen 5 7600", "cores": 6, "ram_gb": 16, "disks": [{"name": "nvme0n1", "size_gb": 500}]}',
   NULL, NULL,
   now() - interval '2 hours', now() - interval '2 hours')

ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- Software Catalog (18 entries across all install methods)
-- ============================================================================

INSERT INTO software_catalog (id, name, description, category, install_method, flatpak_ref, nix_attr, icon_url, approval_required, auto_approve_roles)
VALUES
  ('10000000-0000-0000-0000-000000000001', 'Firefox', 'Open-source web browser with privacy features', 'Browser', 'nix_system', NULL, 'nixpkgs#firefox', NULL, false, '{}'),
  ('10000000-0000-0000-0000-000000000002', 'Chromium', 'Open-source web browser by Google', 'Browser', 'nix_system', NULL, 'nixpkgs#chromium', NULL, true, '{developer,designer}'),
  ('10000000-0000-0000-0000-000000000003', 'Visual Studio Code', 'Lightweight code editor with extension ecosystem', 'Development', 'flatpak', 'app/com.visualstudio.code/x86_64/stable', NULL, NULL, false, '{}'),
  ('10000000-0000-0000-0000-000000000004', 'JetBrains Toolbox', 'Manage JetBrains IDEs (IntelliJ, CLion, etc.)', 'Development', 'flatpak', 'app/com.jetbrains.Toolbox/x86_64/stable', NULL, NULL, true, '{developer}'),
  ('10000000-0000-0000-0000-000000000005', 'Neovim', 'Hyperextensible terminal-based text editor', 'Development', 'nix_user', NULL, 'nixpkgs#neovim', NULL, false, '{}'),
  ('10000000-0000-0000-0000-000000000006', 'GIMP', 'GNU Image Manipulation Program for raster graphics', 'Design', 'flatpak', 'app/org.gimp.GIMP/x86_64/stable', NULL, NULL, false, '{}'),
  ('10000000-0000-0000-0000-000000000007', 'Figma Linux', 'Unofficial Figma client for Linux desktops', 'Design', 'flatpak', 'app/io.github.nickvision.Figma/x86_64/stable', NULL, NULL, true, '{designer}'),
  ('10000000-0000-0000-0000-000000000008', 'Inkscape', 'Professional vector graphics editor', 'Design', 'nix_system', NULL, 'nixpkgs#inkscape', NULL, false, '{}'),
  ('10000000-0000-0000-0000-000000000009', 'Slack', 'Team communication and collaboration platform', 'Communication', 'flatpak', 'app/com.slack.Slack/x86_64/stable', NULL, NULL, false, '{}'),
  ('10000000-0000-0000-0000-000000000010', 'Element', 'Matrix client for secure decentralized messaging', 'Communication', 'flatpak', 'app/im.riot.Riot/x86_64/stable', NULL, NULL, false, '{}'),
  ('10000000-0000-0000-0000-000000000011', 'Signal Desktop', 'End-to-end encrypted messaging app', 'Communication', 'flatpak', 'app/org.signal.Signal/x86_64/stable', NULL, NULL, true, '{}'),
  ('10000000-0000-0000-0000-000000000012', 'LibreOffice', 'Full-featured open-source office productivity suite', 'Office', 'nix_system', NULL, 'nixpkgs#libreoffice', NULL, false, '{}'),
  ('10000000-0000-0000-0000-000000000013', 'Thunderbird', 'Email, calendar, and contacts client', 'Communication', 'nix_system', NULL, 'nixpkgs#thunderbird', NULL, false, '{}'),
  ('10000000-0000-0000-0000-000000000014', 'VLC', 'Versatile open-source media player', 'Media', 'flatpak', 'app/org.videolan.VLC/x86_64/stable', NULL, NULL, false, '{}'),
  ('10000000-0000-0000-0000-000000000015', '1Password', 'Team password manager and secret vault', 'Security', 'flatpak', 'app/com.1password.1Password/x86_64/stable', NULL, NULL, true, '{admin}'),
  ('10000000-0000-0000-0000-000000000016', 'Podman Desktop', 'Container management and development tool', 'Development', 'flatpak', 'app/io.podman_desktop.PodmanDesktop/x86_64/stable', NULL, NULL, true, '{developer,admin}'),
  ('10000000-0000-0000-0000-000000000017', 'Blender', '3D creation suite for modeling, animation, and rendering', 'Design', 'flatpak', 'app/org.blender.Blender/x86_64/stable', NULL, NULL, true, '{designer}'),
  ('10000000-0000-0000-0000-000000000018', 'htop', 'Interactive process viewer for the terminal', 'Utilities', 'nix_user', NULL, 'nixpkgs#htop', NULL, false, '{}')
ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- Software Requests (7 requests in various states)
-- ============================================================================

INSERT INTO software_requests (id, catalog_entry_id, machine_id, username, status, requested_at, resolved_at, resolved_by)
VALUES
  -- Pending: testdev wants JetBrains on eng-ws-01
  ('20000000-0000-0000-0000-000000000001',
   '10000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000002',
   'testdev', 'pending', now() - interval '3 hours', NULL, NULL),

  -- Pending: testuser wants Chromium on kiosk
  ('20000000-0000-0000-0000-000000000002',
   '10000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000006',
   'testuser', 'pending', now() - interval '1 hour', NULL, NULL),

  -- Approved: testdesigner wants Blender on design-ws-01
  ('20000000-0000-0000-0000-000000000003',
   '10000000-0000-0000-0000-000000000017', '00000000-0000-0000-0000-000000000004',
   'testdesigner', 'approved', now() - interval '2 days', now() - interval '2 days', 'testadmin'),

  -- Denied: testuser wants 1Password on kiosk (not authorized)
  ('20000000-0000-0000-0000-000000000004',
   '10000000-0000-0000-0000-000000000015', '00000000-0000-0000-0000-000000000006',
   'testuser', 'denied', now() - interval '5 days', now() - interval '4 days', 'testadmin'),

  -- Installed: testdev has VS Code on eng-ws-01
  ('20000000-0000-0000-0000-000000000005',
   '10000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000002',
   'testdev', 'installed', now() - interval '14 days', now() - interval '14 days', 'testadmin'),

  -- Installed: testdesigner has GIMP on design-ws-01
  ('20000000-0000-0000-0000-000000000006',
   '10000000-0000-0000-0000-000000000006', '00000000-0000-0000-0000-000000000004',
   'testdesigner', 'installed', now() - interval '30 days', now() - interval '30 days', 'testadmin'),

  -- Installing: testdev getting Podman on eng-ws-02
  ('20000000-0000-0000-0000-000000000007',
   '10000000-0000-0000-0000-000000000016', '00000000-0000-0000-0000-000000000003',
   'testdev', 'installing', now() - interval '30 minutes', now() - interval '25 minutes', 'testadmin')

ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- Deployments (4 deployments in various states)
-- ============================================================================

INSERT INTO deployments (id, closure, module_library_ref, instance_data_hash, status, target_filter,
                         total_machines, succeeded, failed,
                         canary_size, batch_size, failure_threshold, rollback_reason,
                         created_at, updated_at)
VALUES
  -- Completed deployment: security patch rollout
  ('30000000-0000-0000-0000-000000000001',
   '/nix/store/abc123def456-nixos-system-hearth-25.05',
   'github:hearth-os/fleet-config@v2.1.0', 'sha256:aabbccdd11223344',
   'completed', '{"role": ["developer", "designer", "default"]}',
   5, 5, 0,
   1, 3, 0.2, NULL,
   now() - interval '7 days', now() - interval '6 days'),

  -- Rolling deployment: feature update in progress
  ('30000000-0000-0000-0000-000000000002',
   '/nix/store/newbuild789ab-nixos-system-hearth-25.05',
   'github:hearth-os/fleet-config@v2.2.0', 'sha256:eeff00112233',
   'rolling', '{"role": ["developer"]}',
   3, 1, 0,
   1, 2, 0.1, NULL,
   now() - interval '2 hours', now() - interval '30 minutes'),

  -- Canary deployment: testing new GNOME config
  ('30000000-0000-0000-0000-000000000003',
   '/nix/store/canary456def-nixos-system-hearth-25.05',
   'github:hearth-os/fleet-config@v2.3.0-rc1', 'sha256:44556677',
   'canary', '{"tags": ["design"]}',
   2, 0, 0,
   1, 1, 0.1, NULL,
   now() - interval '1 hour', now() - interval '45 minutes'),

  -- Failed deployment: bad kernel config
  ('30000000-0000-0000-0000-000000000004',
   '/nix/store/badbuild999-nixos-system-hearth-25.05',
   'github:hearth-os/fleet-config@v2.1.1', 'sha256:bad0bad0',
   'failed', '{"role": ["developer", "designer", "default"]}',
   5, 2, 2,
   1, 3, 0.1, 'Kernel module load failure on 2/5 machines — automatic rollback triggered (failure threshold 10% exceeded)',
   now() - interval '3 days', now() - interval '3 days')

ON CONFLICT (id) DO NOTHING;

-- ============================================================================
-- Deployment Machines (per-machine status for each deployment)
-- ============================================================================

INSERT INTO deployment_machines (deployment_id, machine_id, status, started_at, completed_at, error_message)
VALUES
  -- Completed deployment: all 5 machines succeeded
  ('30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', 'completed', now() - interval '7 days', now() - interval '6 days 23 hours', NULL),
  ('30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002', 'completed', now() - interval '7 days', now() - interval '6 days 22 hours', NULL),
  ('30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000003', 'completed', now() - interval '6 days 23 hours', now() - interval '6 days 21 hours', NULL),
  ('30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000004', 'completed', now() - interval '6 days 23 hours', now() - interval '6 days 20 hours', NULL),
  ('30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000006', 'completed', now() - interval '6 days 22 hours', now() - interval '6 days 19 hours', NULL),

  -- Rolling deployment: 1 done, 2 pending
  ('30000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', 'completed', now() - interval '1 hour', now() - interval '30 minutes', NULL),
  ('30000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000002', 'downloading', now() - interval '10 minutes', NULL, NULL),
  ('30000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000003', 'pending', NULL, NULL, NULL),

  -- Canary deployment: 1 machine testing
  ('30000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000004', 'switching', now() - interval '40 minutes', NULL, NULL),

  -- Failed deployment: 2 succeeded, 2 failed, 1 rolled back
  ('30000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000001', 'completed', now() - interval '3 days', now() - interval '2 days 23 hours', NULL),
  ('30000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000002', 'completed', now() - interval '3 days', now() - interval '2 days 22 hours', NULL),
  ('30000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000003', 'failed', now() - interval '2 days 23 hours', now() - interval '2 days 22 hours', 'nixos-rebuild switch failed: kernel module "vboxdrv" not found in closure'),
  ('30000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000004', 'failed', now() - interval '2 days 22 hours', now() - interval '2 days 21 hours', 'nixos-rebuild switch failed: kernel module "vboxdrv" not found in closure'),
  ('30000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000006', 'rolled_back', now() - interval '2 days 22 hours', now() - interval '2 days 21 hours', NULL)

ON CONFLICT (deployment_id, machine_id) DO NOTHING;

-- ============================================================================
-- User Environments (per-user desktop state on machines)
-- ============================================================================

INSERT INTO user_environments (id, machine_id, username, role, current_closure, target_closure, status, created_at, updated_at)
VALUES
  ('40000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', 'testdev', 'developer',
   '/nix/store/hmdev123-home-manager-generation', '/nix/store/hmdev123-home-manager-generation', 'active',
   now() - interval '14 days', now() - interval '1 hour'),

  ('40000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000002', 'testdev', 'developer',
   '/nix/store/hmdev123-home-manager-generation', '/nix/store/hmdev456-home-manager-generation', 'building',
   now() - interval '30 days', now() - interval '15 minutes'),

  ('40000000-0000-0000-0000-000000000003', '00000000-0000-0000-0000-000000000004', 'testdesigner', 'designer',
   '/nix/store/hmdes789-home-manager-generation', '/nix/store/hmdes789-home-manager-generation', 'active',
   now() - interval '60 days', now() - interval '2 hours'),

  ('40000000-0000-0000-0000-000000000004', '00000000-0000-0000-0000-000000000003', 'testdev', 'developer',
   '/nix/store/hmdev123-home-manager-generation', '/nix/store/hmdev123-home-manager-generation', 'active',
   now() - interval '20 days', now() - interval '3 hours'),

  ('40000000-0000-0000-0000-000000000005', '00000000-0000-0000-0000-000000000006', 'testuser', 'default',
   NULL, '/nix/store/hmuser000-home-manager-generation', 'pending',
   now() - interval '1 day', now() - interval '1 day')

ON CONFLICT (machine_id, username) DO NOTHING;

-- ============================================================================
-- User Configs (per-user environment overrides)
-- ============================================================================

INSERT INTO user_configs (id, username, base_role, overrides, config_hash, latest_closure, build_status, created_at, updated_at)
VALUES
  ('50000000-0000-0000-0000-000000000001', 'testdev', 'developer',
   '{"extra_packages": ["nixpkgs#ripgrep", "nixpkgs#fd"], "git": {"user.name": "Test Developer", "user.email": "testdev@hearth.local"}}',
   'sha256:devconfig001', '/nix/store/hmdev456-home-manager-generation', 'built',
   now() - interval '30 days', now() - interval '15 minutes'),

  ('50000000-0000-0000-0000-000000000002', 'testdesigner', 'designer',
   '{"extra_packages": ["nixpkgs#krita"], "dconf": {"org.gnome.desktop.interface.color-scheme": "prefer-dark"}}',
   'sha256:desconfig001', '/nix/store/hmdes789-home-manager-generation', 'built',
   now() - interval '60 days', now() - interval '2 hours'),

  ('50000000-0000-0000-0000-000000000003', 'testadmin', 'admin',
   '{"extra_packages": ["nixpkgs#tmux", "nixpkgs#bandwhich"]}',
   'sha256:admconfig001', '/nix/store/hmadm012-home-manager-generation', 'built',
   now() - interval '90 days', now() - interval '1 day')

ON CONFLICT (username) DO NOTHING;

-- ============================================================================
-- Compliance Policies
-- ============================================================================

INSERT INTO compliance_policies (id, name, description, nix_expression, severity, control_id, enabled)
VALUES
  ('60000000-0000-0000-0000-000000000001', 'Firewall Enabled',
   'All fleet devices must have the NixOS firewall enabled',
   'config.networking.firewall.enable == true', 'high', 'CIS-3.5.1.1', true),

  ('60000000-0000-0000-0000-000000000002', 'SSH Password Authentication Disabled',
   'Password-based SSH login must be disabled; use key-based auth only',
   'config.services.openssh.settings.PasswordAuthentication == false', 'critical', 'STIG-SV-238332', true),

  ('60000000-0000-0000-0000-000000000003', 'Automatic Updates Enabled',
   'Fleet devices should have automatic NixOS system upgrades enabled',
   'config.system.autoUpgrade.enable == true', 'medium', NULL, true),

  ('60000000-0000-0000-0000-000000000004', 'Full Disk Encryption',
   'All fleet devices must use LUKS full-disk encryption',
   'config.boot.initrd.luks.devices != {}', 'high', 'CIS-1.1.2', true)

ON CONFLICT (name) DO NOTHING;

-- ============================================================================
-- Policy Results (for the completed deployment)
-- ============================================================================

INSERT INTO policy_results (id, deployment_id, machine_id, policy_id, passed, message, evaluated_at)
VALUES
  -- Fleet VM: all pass
  ('70000000-0000-0000-0000-000000000001', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '60000000-0000-0000-0000-000000000001', true, NULL, now() - interval '6 days'),
  ('70000000-0000-0000-0000-000000000002', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '60000000-0000-0000-0000-000000000002', true, NULL, now() - interval '6 days'),
  ('70000000-0000-0000-0000-000000000003', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '60000000-0000-0000-0000-000000000003', true, NULL, now() - interval '6 days'),
  ('70000000-0000-0000-0000-000000000004', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000001', '60000000-0000-0000-0000-000000000004', true, NULL, now() - interval '6 days'),

  -- eng-ws-01: FDE missing
  ('70000000-0000-0000-0000-000000000005', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002', '60000000-0000-0000-0000-000000000001', true, NULL, now() - interval '6 days'),
  ('70000000-0000-0000-0000-000000000006', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002', '60000000-0000-0000-0000-000000000002', true, NULL, now() - interval '6 days'),
  ('70000000-0000-0000-0000-000000000007', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002', '60000000-0000-0000-0000-000000000003', false, 'system.autoUpgrade.enable is false', now() - interval '6 days'),
  ('70000000-0000-0000-0000-000000000008', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000002', '60000000-0000-0000-0000-000000000004', false, 'No LUKS devices configured', now() - interval '6 days'),

  -- design-ws-01: auto-update missing
  ('70000000-0000-0000-0000-000000000009', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000004', '60000000-0000-0000-0000-000000000001', true, NULL, now() - interval '6 days'),
  ('70000000-0000-0000-0000-000000000010', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000004', '60000000-0000-0000-0000-000000000002', true, NULL, now() - interval '6 days'),
  ('70000000-0000-0000-0000-000000000011', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000004', '60000000-0000-0000-0000-000000000003', false, 'system.autoUpgrade.enable is false', now() - interval '6 days'),
  ('70000000-0000-0000-0000-000000000012', '30000000-0000-0000-0000-000000000001', '00000000-0000-0000-0000-000000000004', '60000000-0000-0000-0000-000000000004', true, NULL, now() - interval '6 days')

ON CONFLICT (deployment_id, machine_id, policy_id) DO NOTHING;

-- ============================================================================
-- Audit Events (25 events over the last 7 days)
-- Only insert if seed data doesn't already exist (sentinel: fleet VM machine)
-- ============================================================================

DO $$
BEGIN
  -- Skip if audit events were already seeded (check for a known event)
  IF EXISTS (SELECT 1 FROM audit_events WHERE id = '80000000-0000-0000-0000-000000000001') THEN
    RAISE NOTICE 'Audit events already seeded, skipping';
    RETURN;
  END IF;

  INSERT INTO audit_events (id, event_type, actor, machine_id, details, created_at) VALUES
    -- Machine enrollments
    ('80000000-0000-0000-0000-000000000001', 'machine.enrolled', 'system', '00000000-0000-0000-0000-000000000001', '{"hostname": "hearth-fleet-vm", "method": "pre-enrolled"}', now() - interval '30 days'),
    ('80000000-0000-0000-0000-000000000002', 'machine.enrolled', 'system', '00000000-0000-0000-0000-000000000002', '{"hostname": "eng-ws-01", "method": "iso-enrollment"}', now() - interval '60 days'),
    ('80000000-0000-0000-0000-000000000003', 'machine.enrolled', 'system', '00000000-0000-0000-0000-000000000003', '{"hostname": "eng-ws-02", "method": "iso-enrollment"}', now() - interval '45 days'),
    ('80000000-0000-0000-0000-000000000004', 'machine.enrolled', 'system', '00000000-0000-0000-0000-000000000004', '{"hostname": "design-ws-01", "method": "iso-enrollment"}', now() - interval '90 days'),
    ('80000000-0000-0000-0000-000000000005', 'machine.enrolled', 'system', '00000000-0000-0000-0000-000000000006', '{"hostname": "conference-kiosk-01", "method": "iso-enrollment"}', now() - interval '180 days'),

    -- Machine approvals
    ('80000000-0000-0000-0000-000000000006', 'machine.approved', 'testadmin', '00000000-0000-0000-0000-000000000002', '{"hostname": "eng-ws-01", "role": "developer"}', now() - interval '60 days'),
    ('80000000-0000-0000-0000-000000000007', 'machine.approved', 'testadmin', '00000000-0000-0000-0000-000000000003', '{"hostname": "eng-ws-02", "role": "developer"}', now() - interval '45 days'),
    ('80000000-0000-0000-0000-000000000008', 'machine.approved', 'testadmin', '00000000-0000-0000-0000-000000000004', '{"hostname": "design-ws-01", "role": "designer"}', now() - interval '90 days'),

    -- Deployments
    ('80000000-0000-0000-0000-000000000009', 'deployment.created', 'testadmin', NULL, '{"deployment_id": "30000000-0000-0000-0000-000000000001", "closure": "hearth-25.05", "description": "Security patch rollout"}', now() - interval '7 days'),
    ('80000000-0000-0000-0000-000000000010', 'deployment.completed', 'system', NULL, '{"deployment_id": "30000000-0000-0000-0000-000000000001", "total": 5, "succeeded": 5}', now() - interval '6 days'),
    ('80000000-0000-0000-0000-000000000011', 'deployment.created', 'testadmin', NULL, '{"deployment_id": "30000000-0000-0000-0000-000000000004", "closure": "hearth-25.05-hotfix", "description": "Kernel module update"}', now() - interval '3 days'),
    ('80000000-0000-0000-0000-000000000012', 'deployment.failed', 'system', NULL, '{"deployment_id": "30000000-0000-0000-0000-000000000004", "reason": "Failure threshold exceeded (40% > 10%)"}', now() - interval '3 days'),
    ('80000000-0000-0000-0000-000000000013', 'deployment.created', 'testadmin', NULL, '{"deployment_id": "30000000-0000-0000-0000-000000000002", "closure": "hearth-25.05-v2.2", "description": "Feature update — developer tooling"}', now() - interval '2 hours'),
    ('80000000-0000-0000-0000-000000000014', 'deployment.created', 'testadmin', NULL, '{"deployment_id": "30000000-0000-0000-0000-000000000003", "closure": "hearth-25.05-rc1", "description": "Canary — new GNOME config for design team"}', now() - interval '1 hour'),

    -- Software requests
    ('80000000-0000-0000-0000-000000000015', 'software.requested', 'testdev', '00000000-0000-0000-0000-000000000002', '{"software": "Visual Studio Code", "method": "flatpak"}', now() - interval '14 days'),
    ('80000000-0000-0000-0000-000000000016', 'software.approved', 'testadmin', '00000000-0000-0000-0000-000000000002', '{"software": "Visual Studio Code", "requester": "testdev"}', now() - interval '14 days'),
    ('80000000-0000-0000-0000-000000000017', 'software.requested', 'testdesigner', '00000000-0000-0000-0000-000000000004', '{"software": "GIMP", "method": "flatpak"}', now() - interval '30 days'),
    ('80000000-0000-0000-0000-000000000018', 'software.approved', 'testadmin', '00000000-0000-0000-0000-000000000004', '{"software": "GIMP", "requester": "testdesigner"}', now() - interval '30 days'),
    ('80000000-0000-0000-0000-000000000019', 'software.requested', 'testuser', '00000000-0000-0000-0000-000000000006', '{"software": "1Password", "method": "flatpak"}', now() - interval '5 days'),
    ('80000000-0000-0000-0000-000000000020', 'software.denied', 'testadmin', '00000000-0000-0000-0000-000000000006', '{"software": "1Password", "requester": "testuser", "reason": "Kiosk devices do not require password managers"}', now() - interval '4 days'),

    -- Heartbeat lost
    ('80000000-0000-0000-0000-000000000021', 'machine.heartbeat_lost', 'system', '00000000-0000-0000-0000-000000000005', '{"hostname": "exec-laptop-01", "last_seen": "2026-03-17T10:00:00Z"}', now() - interval '3 days'),

    -- User logins
    ('80000000-0000-0000-0000-000000000022', 'user.login', 'testadmin', NULL, '{"source": "web-console", "ip": "192.168.1.10"}', now() - interval '6 hours'),
    ('80000000-0000-0000-0000-000000000023', 'user.login', 'testdev', NULL, '{"source": "web-console", "ip": "192.168.1.20"}', now() - interval '4 hours'),
    ('80000000-0000-0000-0000-000000000024', 'user.login', 'testdesigner', NULL, '{"source": "web-console", "ip": "192.168.1.30"}', now() - interval '2 hours'),
    ('80000000-0000-0000-0000-000000000025', 'user.login', 'testdev', NULL, '{"source": "greeter", "machine": "eng-ws-01"}', now() - interval '1 hour');

END $$;

COMMIT;
