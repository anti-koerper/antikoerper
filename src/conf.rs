extern crate xdg;

use itertools::Itertools;
use std::io::Read;

use crate::item::{Item, ItemError, ItemErrorKind};
use crate::output::{file::FileOutput, OutputKind};

#[derive(Debug, Clone, Eq, PartialEq)]
enum ConfigErrorKind {
    IoError,
    TomlError,
    ErrorItems,
    DuplicateItem(String),
    Utf8Error,
}

#[derive(Debug)]
pub struct ConfigError {
    kind: ConfigErrorKind,
    cause: Option<Box<dyn (::std::error::Error)>>,
}

impl ::std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        match self.kind {
            ConfigErrorKind::IoError | ConfigErrorKind::TomlError => {
                self.cause.as_ref().unwrap().fmt(f)
            }
            ConfigErrorKind::ErrorItems => write!(f, "some items have errors"),
            ConfigErrorKind::DuplicateItem(ref s) => write!(f, "duplicate key: {}", s),
            ConfigErrorKind::Utf8Error => write!(f, "utf8 error"),
        }
    }
}

impl From<::std::io::Error> for ConfigError {
    fn from(e: ::std::io::Error) -> Self {
        ConfigError {
            kind: ConfigErrorKind::IoError,
            cause: Some(Box::new(e)),
        }
    }
}

impl From<::toml::de::Error> for ConfigError {
    fn from(e: ::toml::de::Error) -> Self {
        ConfigError {
            kind: ConfigErrorKind::TomlError,
            cause: Some(Box::new(e)),
        }
    }
}

impl From<::std::string::FromUtf8Error> for ConfigError {
    fn from(e: ::std::string::FromUtf8Error) -> Self {
        ConfigError {
            kind: ConfigErrorKind::Utf8Error,
            cause: Some(Box::new(e)),
        }
    }
}

impl From<ItemError> for ConfigError {
    fn from(e: ItemError) -> Self {
        ConfigError {
            kind: ConfigErrorKind::ErrorItems,
            cause: Some(Box::new(e)),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub general: General,
    #[serde(default = "output_default")]
    pub output: Vec<OutputKind>,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct General {
    #[serde(default = "shell_default")]
    pub shell: String,
}

fn shell_default() -> String {
    String::from("/usr/bin/sh")
}

fn output_default() -> Vec<OutputKind> {
    vec![OutputKind::File {
        fo: FileOutput::default(),
    }]
}

pub fn load(r: &mut dyn Read) -> Result<Config, ConfigError> {
    let content = {
        let mut buffer = String::new();
        r.read_to_string(&mut buffer)?;
        buffer
    };

    let data: Config = ::toml::de::from_str(&content).map_err(ConfigError::from)?;

    debug!("{:#?}", data);

    if let Some(err) = data
        .items
        .iter()
        .map(|x| x.key.clone())
        .sorted()
        .windows(2)
        .filter_map(|x| {
            if x[0] == x[1] {
                Some(x[0].clone())
            } else {
                None
            }
        })
        .next()
        .map(|n| {
            Err(ConfigError {
                kind: ConfigErrorKind::DuplicateItem(n),
                cause: None,
            })
        })
    {
        return err;
    }

    if let Some(err) = data
        .items
        .iter()
        .filter_map(|i| {
            if i.interval == 0 {
                Some(Err(ItemError::new(
                    i.key.clone(),
                    ItemErrorKind::InvalidInterval,
                )))
            } else {
                None
            }
        })
        .next()
    {
        return err.map_err(ConfigError::from);
    }

    Ok(data)
}

#[cfg(test)]
mod tests {
    extern crate xdg;

    use std::path::PathBuf;

    use crate::conf;
    use crate::output::OutputKind;

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
        match config {
            Err(conf::ConfigError {
                kind: conf::ConfigErrorKind::DuplicateItem(n),
                ..
            }) => {
                assert_eq!(n, "os.uptime");
            }
            _ => panic!("Wrong Error!: {:?}", config),
        }
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
        let xdg_default_dir = match xdg::BaseDirectories::with_prefix("antikoerper")
            .unwrap()
            .create_data_directory(PathBuf::new())
        {
            Ok(s) => s,
            Err(e) => {
                println!("Error: {}", e);
                return;
            }
        };
        match config.output.pop().unwrap() {
            OutputKind::File { fo } => assert_eq!(
                fo.base_path, xdg_default_dir,
                "Expected {:?} to be {:?}",
                fo.base_path, xdg_default_dir
            ),
            _ => {
                println!("Error: wrong OutputKind");
            }
        }
    }
}
