Antikörper
==========

Antikörper is meant to be a lightweight data collection tool.

It's basic idea is to collect data from your PC. You can then simply let it
write to files or send metrics to an influxdb-server!

Possible applications:

- Battery Usage
- Analyze your own PC usage (Which programs are focused when your PC is not
idle?)
- Time spent listening to Music
- Anything you can think of!


Naming
------

The name Antikörper is german for antibody. The idea is that it is there, in the
background, easily forgotten, but nonetheless busy and useful.

Config File
-----------

The config file is a simple toml file that is read at program start. It allows
you to specify which aspects of your Computer should be monitored and in which
intervals.

A sample config with all options used:

```toml
[general]
# shell used for Items with type = "shell"
shell = "/usr/bin/bash"

[[output]]
# writes data to files in the directory /tmp/antikoerper
type = "file"
base_path = "/tmp/antikoerper"
# always write the raw, "undigested" result
always_write_raw = true

[[output]]
type = "influxdb"
# all other options for influxdb are optional/have defaults
url = "http://localhost:8086"
database = "antikoerper"
username = "someuser"
password = "somepassword"
# write raw results in influxdb if no metrics could be parsed
use_raw_as_fallback = false
# write raw results in influxdb
always_write_raw = false

[[items]]
key = "os.battery"
interval = 60
input.type = "command"
input.path = "acpi"
# digest.type = "raw" # no digest

[[items]]
key = "os.usage"
interval = 1
input.type = "file"
input.path = "/proc/loadavg"
digest.type = "regex"
digest.regex = '(?P<load1m>\d+\.\d\d)\s(?P<load5m>\d+\.\d\d)\s(?P<load15m>\d+\.\d\d)'

[[items]]
key = "os.memory"
interval = 60
env = { LANG = "C" }
input.type = "shell"
input.script = "free | grep 'Mem:'"
digest.type = "regex"
digest.regex = '.*\s+(?P<total>\d+)\s+(?P<used>\d+)\s+(?P<free>\d+)\s+(?P<shared>\d+)\s+(?P<cache>\d+)\s+(?P<avail>\d+)"'

[[items]]
key = "workstation.os.procs"
interval = 30
input.type = "command"
input.path = "check_procs"
digest.type = "monitoring-plugin"
```

### Section `general`

- `shell`, the default shell is `/bin/sh`. If you want to use another one,
  specify it here.

### Section/List `output`

- `type = "file"`, write data into files below `base_path`.
- `type = "influxdb"`, write data to a running influxdb-server.

multiple possible, data can be sent to both files and influxdb-servers, and
multiple of those if necessary.

### Section/List `items`

Each item needs to have these keys:
- `key`, the key of the value that the programm will return.
- `interval`, the interval between two 'runs'
- `env`, a table to set environment-variables for input `type`s shell and
  command.
- `input` with `type` either `"file"` OR `"shell"` OR `"command"`.
  - `"file"` takes a `path`
  - `"shell"` takes a `script`
  - `"command"` takes a `path`, and, optionally, an array of `args`
- `digest` with `type` either `"raw"` (the default), `"regex"` or
  `"monitoring-plugin"`.
  - `"regex"` takes a `regex`-String (I recommend using `''` to avoid escapes)
  - `"monitoring-plugin"` may not work for all output of monitoring-plugins


Output
------

The `key`s of Items are the basename for all metrics created by an Item. The
key is extended as follows:
- with `.raw` if the raw-value is written
- `digest.type = "raw"`:
  - with `.parsed` if a f64-value could be parsed
- `digest.type = "regex"`:
  - with `.<named-capture-group>` for every named capture group
    (`(?P<name>...)`) in the provided regex
- `digest.type = "monitoring-plugin"`:
  - with `.<label>` for every label in the performance-metric output of a
    monitoring plugin.
  - with`.<label>.warn` or `.crit` or `.min` or `.max` if the performance-
    metric output of a monitoring plugin provided those.

# LICENSE

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.


--------


__Copyright (C) 2016 Marcel Müller (neikos at neikos.email)__
