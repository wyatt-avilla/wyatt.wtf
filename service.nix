{ self }:
{
  lib,
  config,
  pkgs,
  ...
}:
let
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

    lastfmApiKeyPath = mkOption {
      type = types.str;
      description = "Path to a file containing the Last.fm API key.";
    };

    goodreadsRssUrl = mkOption {
      type = types.str;
      description = "Goodreads updates RSS URL.";
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
  };

  config = mkIf config.services.wyattwtf.enable {
    systemd.services.wyattwtf = {
      inherit description;
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        ExecStart = lib.escapeShellArgs [
          "${bin}"
          "--lastfm-api-key-path"
          config.services.wyattwtf.lastfmApiKeyPath
          "--lastfm-username"
          config.services.wyattwtf.lastfmUsername
          "--letterboxd-rss-url"
          config.services.wyattwtf.letterboxdRssUrl
          "--goodreads-rss-url"
          config.services.wyattwtf.goodreadsRssUrl
        ];
        StateDirectory = "wyattwtf";
        StateDirectoryMode = "0700";
        Restart = "always";
        RestartSec = "5min";
        StartLimitBurst = 1;
        User = "wyattwtf";
        Group = "wyattwtf";
      };

      environment = {
        LEPTOS_SITE_ADDR = "127.0.0.1:${toString config.services.wyattwtf.port}";
      };
    };

    users.users.wyattwtf = {
      isSystemUser = true;
      group = "wyattwtf";
    };

    users.groups.wyattwtf = { };
  };
}
