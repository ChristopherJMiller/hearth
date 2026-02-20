-- Add hardware config, hardware report, and build reproducibility tracking.
--
-- hardware_config stores the generated NixOS hardware-configuration.nix content
-- captured on the device during enrollment (via nixos-generate-config). This is
-- actual Nix code that gets fed into mkFleetHost as the hardware parameter.
--
-- hardware_report stores a JSON summary of detected hardware (CPU, RAM, disk, etc.)
-- for display in the admin console and fleet inventory queries.
--
-- instance_data_hash + module_library_ref enable reproducible builds per the
-- build contract: closure = build(module_library @ git_ref, instance_data_json).

ALTER TABLE machines ADD COLUMN IF NOT EXISTS hardware_config TEXT;
ALTER TABLE machines ADD COLUMN IF NOT EXISTS hardware_report JSONB;
ALTER TABLE machines ADD COLUMN IF NOT EXISTS serial_number TEXT;
ALTER TABLE machines ADD COLUMN IF NOT EXISTS instance_data_hash TEXT;
ALTER TABLE machines ADD COLUMN IF NOT EXISTS module_library_ref TEXT;
