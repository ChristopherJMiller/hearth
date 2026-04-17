# home-modules/admin.nix — Admin/IT role home-manager profile
#
# Environment for system administrators and IT staff. Includes network
# diagnostics, system monitoring, remote management tools, and an
# enhanced shell configuration for administrative work.
{ config, lib, pkgs, ... }:

{
  imports = [ ./common.nix ];

  # --- Terminal: kitty with admin theme ---
  programs.kitty = {
    enable = true;
    settings = {
      font_family = "JetBrainsMono Nerd Font";
      font_size = 10;
      # Dark theme with blue accent for admin distinction
      background = "#0d1117";
      foreground = "#c9d1d9";
      cursor = "#58a6ff";
      selection_background = "#264f78";
      selection_foreground = "#c9d1d9";
      tab_bar_style = "powerline";
      window_padding_width = 2;
      scrollback_lines = 50000;
      confirm_os_window_close = 0;
    };
  };

  # --- Shell: enhanced bash with admin aliases and tools ---
  programs.bash = {
    shellAliases = {
      # System monitoring
      psg = "ps aux | grep -v grep | grep";
      ports = "ss -tulnp";
      listening = "ss -tulnp | grep LISTEN";
      connections = "ss -tunp | grep ESTAB";
      meminfo = "free -h && echo && cat /proc/meminfo | head -5";
      cpuinfo = "lscpu | head -15";
      diskinfo = "lsblk -o NAME,SIZE,TYPE,FSTYPE,MOUNTPOINT && echo && df -h";

      # Systemd shortcuts
      sc = "sudo systemctl";
      scs = "systemctl status";
      scr = "sudo systemctl restart";
      scl = "sudo systemctl list-units --failed";
      jf = "journalctl -f";
      ju = "journalctl -u";

      # Network shortcuts
      pingg = "ping -c 4 8.8.8.8";
      myip = "curl -s ifconfig.me";
      dns = "dig +short";
      tracert = "traceroute";

      # Nix admin
      nrs = "sudo nixos-rebuild switch";
      nrt = "sudo nixos-rebuild test";
      nrb = "sudo nixos-rebuild boot";
      ngen = "sudo nix-env --list-generations --profile /nix/var/nix/profiles/system";
      ngc = "sudo nix-collect-garbage -d";

      # Safety nets
      rm = "rm -i";
      cp = "cp -i";
      mv = "mv -i";
    };
    bashrcExtra = ''
      # Admin environment
      export PATH="$HOME/.local/bin:$PATH"

      # Enhanced prompt with hostname emphasis (useful when SSHing between machines)
      # Starship handles the prompt, but set a fallback
      export PROMPT_COMMAND='echo -ne "\033]0;$(whoami)@$(hostname):$(pwd)\007"'

      # Quick log viewer
      logs() {
        if [ -n "$1" ]; then
          journalctl -u "$1" -f --no-pager
        else
          journalctl -f --no-pager
        fi
      }

      # Quick service status
      svc() {
        systemctl status "$1" 2>/dev/null || echo "Service $1 not found"
      }
    '';
  };

  # Override editor to neovim for admins
  home.sessionVariables = {
    EDITOR = "nvim";
    VISUAL = "nvim";
  };

  # --- Neovim for config editing ---
  programs.neovim = {
    enable = true;
    defaultEditor = true;
    viAlias = true;
    vimAlias = true;
    extraConfig = ''
      set number
      set expandtab
      set tabstop=2
      set shiftwidth=2
      set smartindent
      set termguicolors
      set clipboard=unnamedplus
      set ignorecase
      set smartcase
    '';
  };

  # --- SSH configuration ---
  programs.ssh = {
    enable = true;
    enableDefaultConfig = false;
    matchBlocks = {
      "*" = {
        serverAliveInterval = 60;
        serverAliveCountMax = 3;
        extraOptions = {
          AddKeysToAgent = "yes";
          IdentitiesOnly = "yes";
        };
      };
      "ws-*" = {
        user = "admin";
        forwardAgent = false;
      };
    };
  };

  # --- tmux for persistent sessions ---
  programs.tmux = {
    enable = true;
    clock24 = true;
    historyLimit = 50000;
    keyMode = "vi";
    terminal = "screen-256color";
    extraConfig = ''
      # Better prefix
      unbind C-b
      set -g prefix C-a
      bind C-a send-prefix

      # Split panes with | and -
      bind | split-window -h
      bind - split-window -v

      # Mouse support
      set -g mouse on

      # Status bar
      set -g status-style 'bg=#1e1e2e fg=#cdd6f4'
      set -g status-left '#[fg=#89b4fa,bold] #S '
      set -g status-right '#[fg=#a6adc8] %H:%M %d-%b '
      set -g status-left-length 30

      # Window numbering
      set -g base-index 1
      setw -g pane-base-index 1
      set -g renumber-windows on
    '';
  };

  # --- Git for admin ---
  programs.git.settings = {
    rerere.enabled = true;
  };

  # --- direnv for nix environments ---
  programs.direnv = {
    enable = true;
    enableBashIntegration = true;
    nix-direnv.enable = true;
  };

  # --- fzf ---
  programs.fzf = {
    enable = true;
    enableBashIntegration = true;
  };

  # --- GNOME settings for admin ---
  dconf.settings = {
    "org/gnome/shell" = {
      favorite-apps = [
        "firefox.desktop"
        "kitty.desktop"
        "org.gnome.Nautilus.desktop"
      ] ++ lib.optionals config.hearth.libreoffice.enable [
        "org.libreoffice.LibreOffice.writer.desktop"
      ] ++ lib.optionals config.hearth.chat.enable [
        "element-desktop.desktop"
      ] ++ lib.optionals config.hearth.nextcloud.enable [
        "com.nextcloud.desktopclient.nextcloud.desktop"
      ] ++ lib.optionals config.hearth.thunderbird.enable [
        "thunderbird.desktop"
      ];
    };

    "org/gnome/desktop/background" = {
      picture-options = "zoom";
      color-shading-type = "solid";
      primary-color = "#0d1117";
    };
  };

  # --- Admin packages ---
  home.packages = with pkgs; [
    # System monitoring
    htop
    bottom
    iotop
    sysstat
    dool

    # Network diagnostics
    nmap
    wireshark
    tcpdump
    traceroute
    mtr
    iperf3
    dnsutils
    whois
    netcat-gnu
    socat

    # Remote management
    tmux
    openssh
    ansible
    sshpass

    # Disk and hardware
    smartmontools
    hdparm
    nvme-cli
    lshw
    pciutils
    usbutils

    # Log analysis and text processing
    lnav
    ripgrep
    fd
    bat
    jq
    yq

    # File transfer
    rsync
    rclone

    # Containers
    podman-compose

    # Nix tools
    nix-tree
    nix-diff
    nix-output-monitor

    # Security tools
    openssl
    age
    sops

    # Misc utilities
    httpie
    unzip
    zip
    p7zip
    file
    tree
    ncdu

    # Fonts
    nerd-fonts.jetbrains-mono
  ];

}
