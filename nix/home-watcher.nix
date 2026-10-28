{ config, lib, pkgs, ... }:

let
  cfg = config.services.home-watcher;

  home-watcher-pkg = pkgs.rustPlatform.buildRustPackage rec {
    pname = "home-watcher";
    version = "0.3.0";

    src = ../.;

    cargoLock.lockFile = ../Cargo.lock;

    nativeBuildInputs = [ pkgs.pkg-config ];

    postInstall = ''
      install -Dm644 ${../home-watcher.service} $out/lib/systemd/system/home-watcher.service
    '';

    meta = with lib; {
      description = "Log creation of non-standard home directory files using Linux auditd";
      longDescription = ''
        home-watcher monitors file creation in a user's home directory via
        Linux auditd. It logs which executables create files outside of
        standard XDG directories (config, cache, data, state) and common
        user directories (Downloads, Pictures, etc.).
      '';
      license = licenses.gpl3Plus;
      platforms = platforms.linux;
      maintainers = [];
    };
  };

in {
  options.services.home-watcher = {
    enable = lib.mkEnableOption "home-watcher — log file creation in home directories via auditd";

    user = lib.mkOption {
      type = lib.types.str;
      example = "jdoe";
      description = "The user whose home directory to watch.";
    };

    excludedDirs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [];
      example = [ ".var" "snap" ];
      description = "Additional relative directories inside the home directory to exclude from monitoring";
    };

    excludedExecutables = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [];
      example = [ "dolphin" ];
      description = "Executables to exclude from monitoring entirely (e.g. package managers that write many files)";
    };

    includeDirs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      example = [ ".config" ];
      description = ''
        Directories (relative to the home directory) to watch even though they
        are excluded by default, such as the XDG base directories.
      '';
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = home-watcher-pkg;
      defaultText = lib.literalMD "home-watcher package built from source";
      description = "The home-watcher package to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    security.audit.enable = true;
    security.auditd.enable = true;

    systemd.services.home-watcher = {
      description = "Log creation of non-standard home directory files";
      after = [ "basic.target" "auditd.service" ];
      wants = [ "auditd.service" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        Type = "simple";
        Restart = "on-failure";
        RestartSec = "5s";
        ExecStart = let
          args = lib.concatStringsSep " " (
            [ "${cfg.package}/bin/home-watcher" "--user" cfg.user ]
            ++ [ "--log-file" "/home/${cfg.user}/.local/share/home-watcher.db" ]
            ++ builtins.map (d: "--excluded-dirs ${d}") cfg.excludedDirs
            ++ builtins.map (d: "--include-dirs ${d}") cfg.includeDirs
            ++ builtins.map (e: "--excluded-executables ${e}") cfg.excludedExecutables
          );
        in "${args}";
        User = "root";
        Environment = [
          "HOME_WATCHER_AUDITCTL=${pkgs.audit}/bin/auditctl"
          "PATH=/run/wrappers/bin:${pkgs.coreutils}/bin"
        ];
      };
    };
  };
}