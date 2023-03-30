//! Configuration parsing

use std::io::Read;
use std::path::PathBuf;

use anyhow::{bail, Result};
use itertools::Itertools;
use log::debug;
use serde::Deserialize;

use crate::item::Item;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub general: General,
    #[serde(default = "default_output")]
    pub output: Vec<OutputKind>,
    pub items: Vec<Item>,
}

fn default_output() -> Vec<OutputKind> {
    vec![OutputKind::default()]
}

#[derive(Debug, Clone, Deserialize)]
pub struct General {
    #[serde(default = "shell_default")]
    pub shell: String,
}

fn shell_default() -> String {
    String::from("/bin/sh")
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum OutputKind {
    File {
        base_path: PathBuf,
        #[serde(default)]
        always_write_raw: bool,
    },
    InfluxDB {
        #[serde(default = "influx_url_default")]
        url: String,
        #[serde(default = "influx_database_default")]
        database: String,
        #[serde(flatten)]
        auth: Option<InfluxDBAuth>,
        #[serde(default)]
        use_raw_as_fallback: bool,
        #[serde(default)]
        always_write_raw: bool,
    }, // more in the future?
}

#[derive(Debug, Deserialize)]
pub struct InfluxDBAuth {
    pub username: String,
    pub password: String,
}

fn influx_url_default() -> String {
    String::from("http://localhost:8086")
}

fn influx_database_default() -> String {
    String::from("antikoerper")
}

impl Default for OutputKind {
    fn default() -> Self {
        Self::File {
            base_path: PathBuf::from("/var/log/antikoerper/"),
            always_write_raw: false,
        }
    }
}

pub fn load(r: &mut dyn Read) -> Result<Config> {
    let content = {
        let mut buffer = String::new();
        r.read_to_string(&mut buffer)?;
        buffer
    };

    let data: Config = ::toml::de::from_str(&content)?;

    debug!("{:#?}", data);

    let duplicates = data
        .items
        .iter()
        .map(|x| x.key.clone())
        .sorted()
        .tuple_windows::<(_, _)>()
        .filter_map(|x| if x.0 == x.1 { Some(x.0) } else { None })
        .collect::<Vec<_>>();
    if !duplicates.is_empty() {
        bail!(
            "Configuration contained duplicate keys {}!",
            duplicates.join(", ")
        )
    }

    let interval_too_small = data
        .items
        .iter()
        .filter(|item| item.interval == 0)
        .map(|item| item.key.clone())
        .collect::<Vec<_>>();

    if !interval_too_small.is_empty() {
        bail!(
            "Interval of following items was not bigger than 0: {}",
            interval_too_small.join(", ")
        )
    }

    Ok(data)
}

#[cfg(test)]
mod tests {
    use crate::conf;
    use std::path::PathBuf;

    #[test]
    fn load() {
        let data = r#"[general]
         [[output]]
         type = "file"
         base_path = "/tmp/test"

         [[items]]
         key = "os.uptime"
         interval = 60
         input.type = "shell"
         input.script = "cat /proc/uptime | cut -d' ' -f1"

         [[items]]
         key = "os.loadavg"
         interval = 1
         input.type = "shell"
         input.script = "cat /proc/loadavg | cut -d' ' -f1"
"#;

        let config = conf::load(&mut data.as_bytes()).unwrap();
        assert_eq!(config.items.len(), 2);
    }

    #[test]
    fn no_duplicates() {
        let data = r#"[general]
         [[output]]
         type = "file"
         base_path = "/tmp/test"

         [[items]]
         key = "os.uptime"
         interval = 60
         input.type = "shell"
         input.script = "cat /proc/uptime | cut -d' ' -f1"

         [[items]]
         key = "os.uptime"
         interval = 1
         input.type = "shell"
         input.script = "cat /proc/loadavg | cut -d' ' -f1"
"#;

        let config = conf::load(&mut data.as_bytes());
        assert!(config.is_err());
    }

    #[test]
    fn output_dir() {
        // No output given, default should be used
        let data = r#"[general]
        [[items]]
        key = "os.battery"
        interval = 60
        input.type = "command"
        input.path = "acpi"
        "#;
        let mut config = conf::load(&mut data.as_bytes()).unwrap();
        match config.output.pop().unwrap() {
            conf::OutputKind::File { base_path, .. } => {
                assert_eq!(base_path, PathBuf::from("/var/log/antikoerper"))
            }
            _ => {
                println!("Error: wrong OutputKind");
            }
        }
    }
}
