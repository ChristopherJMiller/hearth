export type InstallMethod = 'nix_system' | 'nix_user' | 'flatpak' | 'home_manager';

export type SoftwareRequestStatus =
  | 'pending'
  | 'approved'
  | 'denied'
  | 'installing'
  | 'installed'
  | 'failed';

export interface CatalogEntry {
  id: string;
  name: string;
  description: string | null;
  category: string | null;
  install_method: InstallMethod;
  flatpak_ref: string | null;
  nix_attr: string | null;
  icon_url: string | null;
  approval_required: boolean;
  auto_approve_roles: string[];
  created_at: string;
}

export interface SoftwareRequest {
  id: string;
  catalog_entry_id: string;
  machine_id: string;
  username: string;
  status: SoftwareRequestStatus;
  requested_at: string;
  resolved_at: string | null;
  resolved_by: string | null;
}
