# modules/compliance/default.nix — Compliance framework for Hearth fleet devices
#
# Imports all individual compliance control modules and provides a profile
# option to enable predefined sets of controls.
{ config, lib, ... }:

let
  cfg = config.services.hearth.compliance;
in
{
  imports = [
    ./cis-1-1-1.nix
    ./cis-3-4-1.nix
    ./cis-4-2-1.nix
    ./stig-v-230223.nix
    ./stig-v-230271.nix
  ];

  options.services.hearth.compliance = {
    profile = lib.mkOption {
      type = lib.types.nullOr (lib.types.enum [ "cis-level1" "cis-level2" "stig" ]);
      default = null;
      description = ''
        Compliance profile to activate. Enables a predefined set of controls:
        - "cis-level1": CIS Benchmark Level 1 controls
        - "cis-level2": CIS Benchmark Level 2 controls (includes Level 1)
        - "stig": DISA STIG controls
      '';
    };
  };

  config = lib.mkMerge [
    # CIS Level 1: basic security controls
    (lib.mkIf (cfg.profile == "cis-level1" || cfg.profile == "cis-level2") {
      services.hearth.compliance."cis-1-1-1".enable = true;
      services.hearth.compliance."cis-3-4-1".enable = true;
      services.hearth.compliance."cis-4-2-1".enable = true;
    })

    # CIS Level 2: adds stricter controls
    (lib.mkIf (cfg.profile == "cis-level2") {
      services.hearth.compliance."stig-v-230271".enable = true;
    })

    # STIG: enables all STIG-mapped controls
    (lib.mkIf (cfg.profile == "stig") {
      services.hearth.compliance."cis-1-1-1".enable = true;
      services.hearth.compliance."cis-3-4-1".enable = true;
      services.hearth.compliance."cis-4-2-1".enable = true;
      services.hearth.compliance."stig-v-230223".enable = true;
      services.hearth.compliance."stig-v-230271".enable = true;
    })
  ];
}
