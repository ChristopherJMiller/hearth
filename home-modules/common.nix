# home-modules/common.nix — Shared home-manager settings for all roles
#
# This module is imported by every role profile and provides the baseline
# configuration that all Hearth-managed users receive. It sets up XDG
# directories, basic shell configuration, Git structure, and GNOME dconf
# settings that apply regardless of role.
{ config, lib, pkgs, ... }:

{
  imports = [ ./chat.nix ./nextcloud.nix ];
  # --- Home-manager basics ---
  home.stateVersion = "25.05";

  # --- XDG directory structure ---
  xdg = {
    enable = true;
    userDirs = {
      enable = true;
      createDirectories = true;
      desktop = "${config.home.homeDirectory}/Desktop";
      documents = "${config.home.homeDirectory}/Documents";
      download = "${config.home.homeDirectory}/Downloads";
      music = "${config.home.homeDirectory}/Music";
      pictures = "${config.home.homeDirectory}/Pictures";
      publicShare = "${config.home.homeDirectory}/Public";
      templates = "${config.home.homeDirectory}/Templates";
      videos = "${config.home.homeDirectory}/Videos";
    };
    # Ensure XDG base directories exist
    configHome = "${config.home.homeDirectory}/.config";
    dataHome = "${config.home.homeDirectory}/.local/share";
    cacheHome = "${config.home.homeDirectory}/.cache";
  };

  # --- Git configuration (structure only, no user-specific values) ---
  programs.git = {
    enable = true;
    # username and email are set per-user by the control plane via
    # the per-user home-manager closure, not in the role profile
    settings = {
      init.defaultBranch = "main";
      pull.rebase = true;
      push.autoSetupRemote = true;
      core = {
        autocrlf = "input";
        editor = "nano";
      };
      merge.conflictStyle = "zdiff3";
      diff.colorMoved = "default";
      # Credential helper — use GNOME Keyring
      credential.helper = "libsecret";
    };
  };

  # --- Shell configuration ---
  programs.bash = {
    enable = true;
    enableCompletion = true;
    historyControl = [ "ignoredups" "ignorespace" ];
    historySize = 10000;
    historyFileSize = 20000;
    shellAliases = {
      ls = "ls --color=auto";
      ll = "ls -la";
      la = "ls -A";
      l = "ls -CF";
      grep = "grep --color=auto";
      ".." = "cd ..";
      "..." = "cd ../..";
      mkdir = "mkdir -pv";
      df = "df -h";
      du = "du -h";
      free = "free -h";
    };
    bashrcExtra = ''
      # Hearth managed workstation
      # History: append, don't overwrite
      shopt -s histappend
      # Check window size after each command
      shopt -s checkwinsize
      # Better globbing
      shopt -s globstar 2>/dev/null
    '';
  };

  # --- Starship prompt ---
  programs.starship = {
    enable = true;
    enableBashIntegration = true;
    settings = {
      format = lib.concatStrings [
        "$username"
        "$hostname"
        "$directory"
        "$git_branch"
        "$git_status"
        "$cmd_duration"
        "$line_break"
        "$character"
      ];
      username = {
        show_always = false;
        format = "[$user]($style)@";
      };
      hostname = {
        ssh_only = true;
        format = "[$hostname]($style) ";
      };
      directory = {
        truncation_length = 3;
        truncate_to_repo = true;
      };
      git_branch = {
        format = "[$symbol$branch]($style) ";
      };
      cmd_duration = {
        min_time = 2000;
        format = "took [$duration]($style) ";
      };
      character = {
        success_symbol = "[>](bold green)";
        error_symbol = "[>](bold red)";
      };
    };
  };

  # --- dconf settings for GNOME ---
  dconf.settings = {
    "org/gnome/desktop/interface" = {
      color-scheme = "prefer-dark";
      font-name = "Cantarell 11";
      document-font-name = "Cantarell 11";
      monospace-font-name = "Source Code Pro 10";
      clock-show-weekday = true;
      enable-hot-corners = false;
    };

    "org/gnome/desktop/peripherals/touchpad" = {
      tap-to-click = true;
      natural-scroll = true;
    };

    "org/gnome/desktop/wm/preferences" = {
      button-layout = "appmenu:minimize,maximize,close";
    };

    "org/gnome/desktop/privacy" = {
      remember-recent-files = true;
      recent-files-max-age = 30;
      remove-old-trash-files = true;
      remove-old-temp-files = true;
    };

    "org/gnome/desktop/screensaver" = {
      lock-enabled = true;
      lock-delay = lib.hm.gvariant.mkUint32 300;
    };
  };

  # --- Common packages for all users ---
  home.packages = with pkgs; [
    # Utilities
    curl
    wget
    unzip
    zip
    file
    tree

    # GNOME integration
    gnome-keyring
    seahorse

    # Fonts (ensure user-level font availability)
    noto-fonts
    noto-fonts-color-emoji
  ];

  # --- GNOME Keyring for credential management ---
  services.gnome-keyring = {
    enable = true;
    components = [ "secrets" "ssh" ];
  };

  # --- Environment variables ---
  home.sessionVariables = {
    EDITOR = "nano";
    VISUAL = "nano";
    PAGER = "less";
    LESS = "-R";
  };
}
