extern crate xdg;

use std::collections::BinaryHeap;
use std::io::Read;
use std::path::PathBuf;

use toml;
use item::Item;

/// The Configuration of Antikoerper
#[derive(Debug, Clone)]
pub struct Config {
    pub items: BinaryHeap<Item>,
    pub general: General,
}

#[derive(Debug, Clone)]
pub struct General {
    pub shell: String,
    pub output: PathBuf,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum ConfigErrorKind {
    IoError,
    TomlError,
    MissingItems,
    ErrorItems,
    DuplicateItem(String),
    MismatchedShellType,
    MismatchedOutputType,
}

#[derive(Debug)]
pub struct ConfigError {
    kind: ConfigErrorKind,
    cause: Option<Box<::std::error::Error>>,
}

impl ::std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        match self.kind {
            ConfigErrorKind::IoError
                | ConfigErrorKind::TomlError => self.cause.as_ref().unwrap().fmt(f),
            ConfigErrorKind::MissingItems => write!(f, "no items section"),
            ConfigErrorKind::ErrorItems => write!(f, "some items have errors"),
            ConfigErrorKind::DuplicateItem(ref s) => write!(f, "duplicate key: {}", s),
            ConfigErrorKind::MismatchedShellType => write!(f, "general.shell has to be a string"),
            ConfigErrorKind::MismatchedOutputType => write!(f, "general.output has to be a path")
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

impl From<toml::ParserError> for ConfigError {
    fn from(e: toml::ParserError) -> Self {
        ConfigError {
            kind: ConfigErrorKind::TomlError,
            cause: Some(Box::new(e)),
        }
    }
}

pub fn load(r: &mut Read, o: PathBuf) -> Result<Config, ConfigError> {
    let content = {
        let mut buffer = String::new();
        try!(r.read_to_string(&mut buffer));
        buffer
    };


    let mut parser = toml::Parser::new(&content);
    let parsed = if let Some(t) = parser.parse() {
        t
    } else {
        return Err(ConfigError::from(parser.errors[0].clone()));
    };

    debug!("{:#?}", parsed);

    let general = match parsed.get("general") {
        Some(&toml::Value::Table(ref v)) => {
            General {
                shell: match v.get("shell") {
                    Some(&toml::Value::String(ref s)) => s.clone(),
                    Some(_) => return Err(ConfigError {
                        kind: ConfigErrorKind::MismatchedShellType,
                        cause: None,
                    }),
                    _ => String::from("/usr/bin/sh"),
                },
                // The function create_data_directory creates relative paths as subdirectories
                // to XDG_DATA_HOME/antikoerper/.
                // If the given path is absolute, the path will be overwritten, no usage of the
                // XDG environment variables in this case
                // If this functions returns successfully, the path in general.output definitely exists.
                output : try!(xdg::BaseDirectories::with_prefix("antikoerper")
                    .unwrap()
                    .create_data_directory(if o == PathBuf::new() {
                        match v.get("output") {
                            Some(&toml::Value::String(ref s)) => PathBuf::from(s.clone()),
                            Some(_) => return Err(ConfigError {
                                kind: ConfigErrorKind::MismatchedOutputType,
                                cause: None,
                            }),
                            // if it is not given either way, we just use the empty one
                            _ => o.clone(),
                        }
                    } else {
                         // using the one provided with commandline argument
                         o
                    },)
                    .map_err(|e| {
                        println!("Error while checking/creating path");
                        println!("Error: {}", e);
                        ConfigError {
                            kind: ConfigErrorKind::IoError,
                            cause: None,
                        }
                    })),
            }
        },

        _ => General {
            shell: String::from("/usr/bin/sh"),
            output: o,
        }
    };

    trace!("Output path is: {:#?}", general.output);

    let items = match parsed.get("items") {
        Some(&toml::Value::Array(ref t)) => t,
        _ => return Err(ConfigError {
            kind: ConfigErrorKind::MissingItems,
            cause: None
        })
    }.iter().filter_map(|v| if let toml::Value::Table(ref v) = *v {
        Some(Item::from_toml(v))
    } else {
        None
    }).collect::<Vec<_>>();

    for err in items.iter().filter(|x| x.is_err()) {
        if let Err(ref x) = *err {
            println!("{}", x);
        }
    }

    if items.iter().filter(|x| x.is_err()).count() > 0 {
        return Err(ConfigError {
            kind: ConfigErrorKind::ErrorItems,
            cause: Some(Box::new(items.iter().filter_map(|x| x.clone().err()).next().unwrap()))
        });
    }

    let mut it = items.iter().map(|x| x.as_ref().unwrap().key.clone()).collect::<Vec<_>>();
    it.sort();
    let mut it = it.windows(2).map(|x| if x[0] == x[1] { Some(x[0].clone()) } else { None })
        .filter_map(|x| x);

    if let Some(n) = it.next() {
        return Err(ConfigError {
            kind: ConfigErrorKind::DuplicateItem(n),
            cause: None
        })
    }


    Ok(Config {
        items: BinaryHeap::from(items.iter().cloned().map(|x| x.unwrap()).collect::<Vec<_>>()),
        general: general,
    })
}

#[cfg(test)]
mod tests {
    extern crate xdg;

    use std::path::PathBuf;

    use conf;

    #[test]
    fn load() {
        let data = "[[items]]
         key = \"os.uptime\"
         interval = 60
         shell = \"cat /proc/uptime | cut -d\' \' -f1\"

         [[items]]
         key = \"os.loadavg\"
         interval = 1
         shell = \"cat /proc/loadavg | cut -d\' \' -f1\"
";

        let config = conf::load(&mut data.as_bytes(), PathBuf::from("")).unwrap();
        assert_eq!(config.items.len(), 2);
    }

    #[test]
    fn no_duplicates() {
        let data = "[[items]]
         key = \"os.uptime\"
         interval = 60
         shell = \"cat /proc/uptime | cut -d\' \' -f1\"

         [[items]]
         key = \"os.uptime\"
         interval = 1
         shell = \"cat /proc/loadavg | cut -d\' \' -f1\"
";

        let config = conf::load(&mut data.as_bytes(), PathBuf::from(""));
        assert!(config.is_err());
        match config {
            Err(conf::ConfigError{ kind: conf::ConfigErrorKind::DuplicateItem(n), ..}) => {
                assert_eq!(n, "os.uptime");
            },
            _ => {
                panic!("Wrong Error!")
            }
        }
    }

    #[test]
    fn output_dir() {
        let data = "[general]
        output = \"/tmp/test\"
        [[items]]
        key = \"os.battery\"
        interval = 60
        shell = \"acpi\"
        ";
        // Testcase 1: output-dir supplied by config file only
        let config = conf::load(&mut data.as_bytes(), PathBuf::new()).unwrap();
        assert_eq!(config.general.output, PathBuf::from("/tmp/test"));

        // Testcase 2: output-dir supplied by config, but also with commandline-argument
        // argument should override config
        let config = conf::load(&mut data.as_bytes(), PathBuf::from("/tmp/cmd_test")).unwrap();
        assert_eq!(config.general.output, PathBuf::from("/tmp/cmd_test"));

        // Testcase 3: Not output given, default should be used
        let data = "[general]
        [[items]]
        key = \"os.battery\"
        interval = 60
        shell = \"acpi\"
        ";
        let config = conf::load(&mut data.as_bytes(), PathBuf::new()).unwrap();
        let xdg_default_dir = match xdg::BaseDirectories::with_prefix("antikoerper").unwrap()
            .create_data_directory(&PathBuf::new()) {
                Ok(s) => s,
                Err(e) => {
                    println!("Error: {}", e);
                    return;
                }
            };
        assert_eq!(config.general.output, xdg_default_dir);
    }
}
