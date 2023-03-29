{ pkgs ? import <nixpkgs> {}, ...}:
let
  mp = pkgs.monitoring-plugins;
  toml = pkgs.formats.toml {};
in
{
  cfgFile = toml.generate "example.toml" {
    general = {
      shell = "/bin/sh";
    };
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
          path = "${mp}/bin/check_procs";
        };
        digest.type = "monitoring-plugin";
      }
    ];
  };
}

