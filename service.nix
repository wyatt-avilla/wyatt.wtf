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
  };

  config = mkIf config.services.wyattwtf.enable {
    systemd.services.wyattwtf = {
      inherit description;
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];

      serviceConfig = {
        ExecStart = lib.concatStringsSep " " [ "${bin}" ];
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
