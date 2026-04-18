-- Hearth Demo Environment: Seed Data
-- Idempotent — safe to run multiple times via ON CONFLICT DO NOTHING.
-- Run after migrations: psql $DATABASE_URL -f dev/seed-demo-data.sql
--
-- NOTE: this script intentionally does NOT seed any fake machines, deployments,
-- audit events, software requests, or policy results. Machines must be real —
-- enroll a fleet VM (`just fleet-vm`) or run `just enroll <name>` to populate
-- those tables. Only static reference data (software catalog, compliance
-- policies) is seeded here.

BEGIN;

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
-- User Environment Configs (triggers build pipeline for per-user closures)
-- ============================================================================
-- Seed configs for the demo Kanidm users so the build worker can produce
-- per-user home-manager closures before first login.
-- config_hash = sha256(base_role || '|' || overrides_json) matches the Rust
-- compute_user_config_hash() function in repo.rs.

INSERT INTO user_configs (username, base_role, overrides, config_hash, build_status)
VALUES
  ('testuser@kanidm.hearth.local',     'default',   '{}', encode(sha256(convert_to('default|{}',   'UTF8')), 'hex'), 'pending'),
  ('testadmin@kanidm.hearth.local',    'admin',     '{}', encode(sha256(convert_to('admin|{}',     'UTF8')), 'hex'), 'pending'),
  ('testdev@kanidm.hearth.local',      'developer', '{}', encode(sha256(convert_to('developer|{}', 'UTF8')), 'hex'), 'pending'),
  ('testdesigner@kanidm.hearth.local', 'designer',  '{}', encode(sha256(convert_to('designer|{}',  'UTF8')), 'hex'), 'pending')
ON CONFLICT (username) DO UPDATE SET
  build_status = 'pending',
  build_error = NULL,
  updated_at = now();

COMMIT;
