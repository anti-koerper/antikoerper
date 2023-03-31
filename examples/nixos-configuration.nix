{ pkgs, ... }:

let
  antikoerper = pkgs.rustPlatform.buildRustPackage rec {
    pname = "antikoerper";
    version = "0.3.0";
    src = pkgs.fetchCrate {
      inherit pname version;
      sha256 = "sha256-N65QTeX381VANFDh6fmKrwUoa2572mPwh/HusHSypcU=";
    };
    cargoSha256 = "sha256-Ft4cEjM7VvTPEXtygBVTi8s6t5C7wDrrdn1FvOvGZpc=";
  };
  monplug = pkgs.monitoring-plugins;
  antikoerper-config = (pkgs.formats.toml {}).generate "antikoerper-config.toml" {
    general.shell = "/bin/sh";
    output = [
      {
        type = "file";
        base_path = "/tmp/antikoerper";
      }
    ];
    items = [
      {
        key = "os.load";
        interval = 10;
        input.type = "file";
        input.path = "/proc/loadavg";
        digest.type = "regex";
        digest.regex = ''(?P<load1m>\d+\.\d\d)\s(?P<load5m>\d+\.\d\d)\s(?P<load15m>\d+\.\d\d)'';
      }
      {
        key = "os.procs";
        interval = 30;
        input = {
          type = "command";
          path = "${monplug}/bin/check_procs";
        };
        digest.type = "monitoring-plugin";
      }
  ];
};

in
{
  systemd.services.antikoerper = {
    enable = true;
    after = [ "network.target" ];
    description = "antikoerper daemon";
    environment.RUST_LOG = "info";
    script = "${antikoerper}/bin/antikoerper -c ${antikoerper-config}";
    serviceConfig = {
      Type = "simple";
      Restart = "always";
    };
    wantedBy = [ "multi-user.target" ];
  };
}
