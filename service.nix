{ self }:
{
  lib,
  config,
  pkgs,
  ...
}:
let
  cfg = config.services.wyattwtf;
  bin = lib.getExe self.packages.${pkgs.system}.default;
  description = "wyatt.wtf webservice";
in
with lib;
{
  options.services.wyattwtf = {
    enable = mkEnableOption description;

    port = mkOption {
      type = types.int;
      default = 8080;
      description = "Port for the wyattwtf service to listen on.";
    };

    user = mkOption {
      type = types.str;
      default = "wyattwtf";
      description = ''
        User account under which the wyattwtf service runs.

        The default user is created by this module. If you set this to another
        value, define the user elsewhere in your NixOS configuration.
      '';
    };

    group = mkOption {
      type = types.str;
      default = "wyattwtf";
      description = ''
        Group under which the wyattwtf service runs.

        The default group is created by this module. If you set this to another
        value, define the group elsewhere in your NixOS configuration.
      '';
    };

    lastfmApiKeyPath = mkOption {
      type = types.str;
      description = "Path to a file containing the Last.fm API key.";
    };

    goodreadsRssUrlPath = mkOption {
      type = types.str;
      description = "Path to a file containing the Goodreads updates RSS URL.";
    };

    lastfmUsername = mkOption {
      type = types.str;
      default = "wyattwtf";
      description = "Last.fm username to fetch recent tracks for.";
    };

    letterboxdRssUrl = mkOption {
      type = types.str;
      default = "https://letterboxd.com/wyattwtf/rss/";
      description = "Letterboxd RSS URL.";
    };

    upstreamTimeoutSeconds = mkOption {
      type = types.ints.positive;
      default = 10;
      description = "Total timeout, in seconds, for upstream feed requests.";
    };
  };

  config = mkIf cfg.enable {
    systemd.services.wyattwtf = {
      inherit description;
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        ExecStart = lib.escapeShellArgs [
          "${bin}"
          "--lastfm-api-key-path"
          cfg.lastfmApiKeyPath
          "--lastfm-username"
          cfg.lastfmUsername
          "--letterboxd-rss-url"
          cfg.letterboxdRssUrl
          "--goodreads-rss-url-path"
          cfg.goodreadsRssUrlPath
          "--upstream-timeout-seconds"
          (toString cfg.upstreamTimeoutSeconds)
        ];
        StateDirectory = "wyattwtf";
        StateDirectoryMode = "0700";
        Restart = "always";
        RestartSec = "5min";
        StartLimitBurst = 1;
        User = cfg.user;
        Group = cfg.group;
      };

      environment = {
        LEPTOS_SITE_ADDR = "127.0.0.1:${toString cfg.port}";
      };
    };

    users.users = optionalAttrs (cfg.user == "wyattwtf") {
      wyattwtf = {
        isSystemUser = true;
        inherit (cfg) group;
      };
    };

    users.groups = optionalAttrs (cfg.group == "wyattwtf") { wyattwtf = { }; };
  };
}
