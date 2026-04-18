# home-modules/developer.nix — Developer role home-manager profile
#
# Environment for software developers. Includes enhanced terminal setup,
# development editors, language toolchains, and container tools.
{ config, lib, pkgs, ... }:

{
  imports = [ ./common.nix ];

  # --- Terminal: kitty ---
  programs.kitty = {
    enable = true;
    settings = {
      font_family = "JetBrainsMono Nerd Font";
      font_size = 11;
      bold_font = "auto";
      italic_font = "auto";
      # Theme: dark with warm accents (matches Hearth branding)
      background = "#1e1e2e";
      foreground = "#cdd6f4";
      cursor = "#f5e0dc";
      selection_background = "#585b70";
      selection_foreground = "#cdd6f4";
      # Tab bar
      tab_bar_style = "powerline";
      tab_powerline_style = "slanted";
      # Window
      window_padding_width = 4;
      confirm_os_window_close = 0;
      # Scrollback
      scrollback_lines = 10000;
      # URLs
      url_style = "curly";
      detect_urls = true;
    };
  };

  # --- Shell: enhanced bash with dev aliases ---
  programs.bash = {
    shellAliases = {
      g = "git";
      gs = "git status";
      gl = "git log --oneline -20";
      gd = "git diff";
      gco = "git checkout";
      gcm = "git commit -m";
      gp = "git push";
      gpl = "git pull";

      # Container aliases
      dk = "podman";
      dkps = "podman ps";
      dkc = "podman-compose";

      # Nix aliases
      nr = "nix run";
      nb = "nix build";
      nd = "nix develop";
      nf = "nix flake";
    };
    bashrcExtra = ''
      # Developer environment
      # Add local bin to path
      export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"

      # Better history search with Ctrl+R
      bind '"\e[A": history-search-backward' 2>/dev/null
      bind '"\e[B": history-search-forward' 2>/dev/null
    '';
  };

  # Override editor to neovim for developers
  home.sessionVariables = {
    EDITOR = "nvim";
    VISUAL = "nvim";
  };

  # --- Neovim ---
  programs.neovim = {
    enable = true;
    defaultEditor = true;
    viAlias = true;
    vimAlias = true;
    extraConfig = ''
      set number
      set relativenumber
      set expandtab
      set tabstop=2
      set shiftwidth=2
      set smartindent
      set termguicolors
      set signcolumn=yes
      set updatetime=250
      set clipboard=unnamedplus
      set ignorecase
      set smartcase
      set undofile
    '';
  };

  # --- Git: developer-specific extras ---
  programs.git.settings = {
    rerere.enabled = true;
    column.ui = "auto";
    branch.sort = "-committerdate";
    fetch.prune = true;
  };

  # --- Delta (git diff pager) ---
  programs.delta = {
    enable = true;
    enableGitIntegration = true;
    options = {
      navigate = true;
      line-numbers = true;
      syntax-theme = "Monokai Extended";
    };
  };

  # --- direnv for per-project environments ---
  programs.direnv = {
    enable = true;
    enableBashIntegration = true;
    nix-direnv.enable = true;
  };

  # --- fzf for fuzzy finding ---
  programs.fzf = {
    enable = true;
    enableBashIntegration = true;
    defaultOptions = [
      "--height 40%"
      "--layout=reverse"
      "--border"
    ];
  };

  # --- GNOME settings for developers ---
  dconf.settings = {
    "org/gnome/shell" = {
      favorite-apps = [
        "firefox.desktop"
        "codium.desktop"
        "kitty.desktop"
        "org.gnome.Nautilus.desktop"
      ] ++ lib.optionals config.hearth.chat.enable [
        "org.gnome.Fractal.desktop"
      ] ++ lib.optionals config.hearth.nextcloud.enable [
        "com.nextcloud.desktopclient.nextcloud.desktop"
      ] ++ lib.optionals config.hearth.thunderbird.enable [
        "thunderbird.desktop"
      ];
      enabled-extensions = [
        "appindicatorsupport@rgcjonas.gmail.com"
        "dash-to-panel@jderose9.github.com"
      ];
    };

    "org/gnome/desktop/background" = {
      picture-options = "zoom";
      color-shading-type = "solid";
      primary-color = "#1e1e2e";
    };
  };

  # --- Developer packages ---
  home.packages = with pkgs; [
    # Editors
    vscodium

    # Version control
    git
    git-lfs
    gh

    # Build tools
    gcc
    gnumake
    cmake
    pkg-config

    # Languages
    python3
    nodejs
    rustup

    # Container tools
    podman-compose

    # Debugging & profiling
    gdb
    strace

    # Search & navigation
    ripgrep
    fd
    bat
    eza
    jq
    yq

    # HTTP clients
    httpie
    curl

    # TUI tools
    lazygit
    bottom

    # Nix tools
    nix-tree
    nix-diff
    nixpkgs-fmt

    # Fonts
    nerd-fonts.jetbrains-mono
    nerd-fonts.fira-code

    # GNOME extensions
    gnomeExtensions.appindicator
  ];
}
