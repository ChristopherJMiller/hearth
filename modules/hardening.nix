# modules/hardening.nix — NixOS module for security hardening baseline
#
# Provides two levels of hardening: "standard" for typical enterprise desktops
# and "strict" for high-security environments. Uses NixOS native security
# options wherever possible.
{ config, lib, pkgs, ... }:

let
  cfg = config.services.hearth.hardening;
  isStrict = cfg.level == "strict";
in
{
  options.services.hearth.hardening = {
    enable = lib.mkEnableOption "Hearth security hardening baseline";

    level = lib.mkOption {
      type = lib.types.enum [ "standard" "strict" ];
      default = "standard";
      description = ''
        Hardening level:
        - "standard": Reasonable enterprise defaults. Disables root login,
          restricts ptrace, enables audit, configures firewall defaults,
          restricts kernel module loading.
        - "strict": Everything in standard plus AppArmor, restricted
          unprivileged user namespaces, and lockdown mode.
      '';
    };
  };

  config = lib.mkIf cfg.enable (lib.mkMerge [
    # ===== Standard hardening (always applied when enabled) =====
    {
      # --- Disable root login ---
      # Root password is disabled; use sudo for administrative tasks
      security.sudo = {
        enable = true;
        wheelNeedsPassword = true;
        extraConfig = ''
          # Hearth hardening: require password for sudo, log all commands
          Defaults    log_output
          Defaults    log_input
          Defaults    timestamp_timeout=15
          Defaults    passwd_tries=3
        '';
      };

      # Disable root SSH login
      services.openssh = {
        settings = {
          PermitRootLogin = lib.mkForce "no";
          PasswordAuthentication = lib.mkDefault false;
        };
      };

      # --- Restrict ptrace ---
      # Only allow ptrace by parent processes (no cross-process debugging)
      boot.kernel.sysctl = {
        "kernel.yama.ptrace_scope" = 1;

        # Restrict dmesg to root
        "kernel.dmesg_restrict" = 1;

        # Hide kernel pointers from non-root
        "kernel.kptr_restrict" = 1;

        # Restrict performance events
        "kernel.perf_event_paranoid" = 3;

        # Disable SysRq except for sync and reboot
        "kernel.sysrq" = 176;

        # Network hardening
        "net.ipv4.conf.all.rp_filter" = 1;
        "net.ipv4.conf.default.rp_filter" = 1;
        "net.ipv4.conf.all.accept_redirects" = 0;
        "net.ipv4.conf.default.accept_redirects" = 0;
        "net.ipv6.conf.all.accept_redirects" = 0;
        "net.ipv6.conf.default.accept_redirects" = 0;
        "net.ipv4.conf.all.send_redirects" = 0;
        "net.ipv4.conf.default.send_redirects" = 0;
        "net.ipv4.icmp_echo_ignore_broadcasts" = 1;
        "net.ipv4.tcp_syncookies" = 1;
        "net.ipv4.tcp_timestamps" = 0;
      };

      # --- Firewall ---
      networking.firewall = {
        enable = true;
        # Default: deny all inbound, allow all outbound
        allowedTCPPorts = [ ];
        allowedUDPPorts = [ ];
        # Log dropped packets for audit
        logRefusedConnections = true;
        logRefusedPackets = true;
      };

      # --- Audit framework ---
      security.auditd.enable = true;
      security.audit = {
        enable = true;
        rules = [
          # Log all execve calls (program execution)
          "-a always,exit -F arch=b64 -S execve -k exec"
          # Log changes to authentication configuration
          "-w /etc/pam.d/ -p wa -k pam_changes"
          "-w /etc/shadow -p wa -k shadow_changes"
          "-w /etc/passwd -p wa -k passwd_changes"
          "-w /etc/group -p wa -k group_changes"
          # Log sudo usage
          "-w /var/log/sudo.log -p wa -k sudo_log"
          # Log Nix store modifications
          "-w /nix/store -p w -k nix_store"
        ];
      };

      # --- Restrict kernel module loading ---
      # Blacklist uncommon network protocols that increase attack surface
      boot.blacklistedKernelModules = [
        "dccp"
        "sctp"
        "rds"
        "tipc"
        "n-hdlc"
        "ax25"
        "netrom"
        "x25"
        "rose"
        "decnet"
        "econet"
        "af_802154"
        "ipx"
        "appletalk"
        "psnap"
        "p8023"
        "p8022"
        "can"
        "atm"
        # Uncommon filesystems
        "cramfs"
        "freevxfs"
        "jffs2"
        "hfs"
        "hfsplus"
        "udf"
        # Firewire / Thunderbolt DMA (physical attack vector)
        "firewire-core"
        "thunderbolt"
      ];

      # --- Misc hardening ---
      # Disable core dumps
      security.pam.loginLimits = [
        { domain = "*"; type = "hard"; item = "core"; value = "0"; }
      ];

      # Restrict access to /proc
      security.protectKernelImage = true;

      # Use a hardened kernel if available
      # boot.kernelPackages = pkgs.linuxPackages_hardened;
      # ^ Uncommented by fleet operators who want the hardened kernel.
      # The default kernel with sysctl hardening is a reasonable baseline.

      # Ensure NixOS garbage collection doesn't remove active system generations
      nix.gc = {
        automatic = true;
        dates = "weekly";
        options = "--delete-older-than 30d";
      };
    }

    # ===== Strict hardening (additional measures) =====
    (lib.mkIf isStrict {
      # --- AppArmor ---
      security.apparmor = {
        enable = true;
        # Use default AppArmor profiles from nixpkgs
        packages = with pkgs; [
          apparmor-profiles
        ];
      };

      # --- Restrict unprivileged user namespaces ---
      # This breaks some Flatpak/bubblewrap sandboxing, so only in strict mode
      boot.kernel.sysctl = {
        "kernel.unprivileged_userns_clone" = 0;
      };

      # --- Kernel lockdown mode ---
      # Prevents runtime modification of the running kernel
      boot.kernelParams = [ "lockdown=confidentiality" ];

      # --- Stricter mount options ---
      fileSystems = {
        "/tmp" = {
          device = "tmpfs";
          fsType = "tmpfs";
          options = [ "nosuid" "nodev" "noexec" "mode=1777" "size=4G" ];
        };
      };

      # --- Additional sysctl hardening ---
      boot.kernel.sysctl = {
        # Disable bpf() for unprivileged users
        "kernel.unprivileged_bpf_disabled" = 1;
        # Harden BPF JIT
        "net.core.bpf_jit_harden" = 2;
      };

      # --- Restrict USB mass storage (data exfiltration prevention) ---
      boot.blacklistedKernelModules = [
        "usb-storage"
        "uas"
      ];

      # --- Stricter SSH ---
      services.openssh.settings = {
        X11Forwarding = false;
        AllowAgentForwarding = false;
        AllowTcpForwarding = false;
        MaxAuthTries = 3;
        ClientAliveInterval = 300;
        ClientAliveCountMax = 2;
      };

      # --- Audit: more verbose in strict mode ---
      security.audit.rules = [
        # Log all file opens (noisy but comprehensive)
        "-a always,exit -F arch=b64 -S open,openat -F exit=-EACCES -k access_denied"
        "-a always,exit -F arch=b64 -S open,openat -F exit=-EPERM -k access_denied"
        # Log privilege escalation attempts
        "-a always,exit -F arch=b64 -S setuid -S setgid -S setreuid -S setregid -k privilege_escalation"
        # Log kernel module operations
        "-a always,exit -F arch=b64 -S init_module -S delete_module -k kernel_modules"
      ];
    })
  ]);
}
