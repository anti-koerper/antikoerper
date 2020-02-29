use std::collections::BTreeMap;
use std::error::Error;
use std::path::PathBuf;

use serde_regex;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ItemErrorKind {
    InvalidInterval,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ItemError {
    key: String,
    kind: ItemErrorKind,
}

impl ItemError {
    fn as_str(&self) -> &str {
        match self.kind {
            ItemErrorKind::InvalidInterval => {
                "interval has to be bigger than 0 and smaller than MAX_INT64"
            }
        }
    }
}

impl ItemError {
    pub fn new(key: String, k: ItemErrorKind) -> ItemError {
        ItemError { key: key, kind: k }
    }
}

impl Error for ItemError {
    fn description(&self) -> &str {
        self.as_str()
    }
}

impl ::std::fmt::Display for ItemError {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> Result<(), ::std::fmt::Error> {
        write!(f, "{}: {}", self.key, self.as_str())
    }
}

/// The different kinds of items one can supervise
#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ItemKind {
    /// Read the file at the given location, useful on Linux for the /sys dir for example
    File { path: PathBuf },
    /// Path to an executable with a list of arguments to be given to the executable
    Command {
        path: PathBuf,
        #[serde(default)]
        args: Vec<String>,
    },
    /// A string to be executed in a shell context
    Shell { script: String },
}

/// A single item, knowing when it is supposed to run next, what should be done and its key.
#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    pub interval: i64,
    pub key: String,

    #[serde(default)]
    pub env: BTreeMap<String, String>,

    #[serde(rename = "input")]
    pub kind: ItemKind,

    #[serde(default)]
    pub digest: DigestKind,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DigestKind {
    Regex {
        #[serde(with = "serde_regex")]
        regex: ::regex::Regex,
    },
    #[serde(rename = "none")]
    Raw,
    // Maybe later more?
}

impl Default for DigestKind {
    fn default() -> DigestKind {
        DigestKind::Raw
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use toml;

    use crate::item::{Item, ItemKind};

    #[test]
    fn deser_item() {
        let item_str = r#"
            key = "os.loadavg"
            interval = 10
            input.type = "file"
            input.path = "/proc/loadavg"
        "#;
        let item_deser: Result<Item, _> = toml::from_str(item_str);
        assert!(item_deser.is_ok());
        let item = item_deser.unwrap();
        assert_eq!(item.key, "os.loadavg");
        assert_eq!(item.interval, 10);
    }

    #[test]
    fn deser_itemkind_file() {
        let item_str = r#"
            key = "os.loadavg"
            interval = 10
            input.type = "file"
            input.path = "/proc/loadavg"
        "#;
        let item_deser: Result<Item, _> = toml::from_str(item_str);
        assert!(item_deser.is_ok());
        let item = item_deser.unwrap();
        assert_eq!(
            item.kind,
            ItemKind::File {
                path: PathBuf::from("/proc/loadavg")
            }
        );
    }

    #[test]
    fn deser_itemkind_shell() {
        let item_str = r#"
            key = "os.loadavg"
            interval = 10
            input.type = "shell"
            input.script = "df /var | tail -1"
        "#;
        let item_deser: Result<Item, _> = toml::from_str(item_str);
        assert!(item_deser.is_ok());
        let item = item_deser.unwrap();
        assert_eq!(
            item.kind,
            ItemKind::Shell {
                script: String::from("df /var | tail -1")
            }
        );
    }

    #[test]
    fn deser_itemkind_command_without_args() {
        let item_str = r#"
            key = "os.battery"
            interval = 60
            input.type = "command"
            input.path = "acpi"
        "#;
        let item_deser: Result<Item, _> = toml::from_str(item_str);
        assert!(item_deser.is_ok());
        let item = item_deser.unwrap();
        assert_eq!(
            item.kind,
            ItemKind::Command {
                path: PathBuf::from("acpi"),
                args: Vec::new()
            }
        );
    }

    #[test]
    fn deser_itemkind_command_with_args() {
        let item_str = r#"
            key = "os.battery"
            interval = 60
            input.type = "command"
            input.path = "acpi"
            input.args = [ "-b", "-i" ]
        "#;
        let item_deser: Result<Item, _> = toml::from_str(item_str);
        assert!(item_deser.is_ok());
        let item = item_deser.unwrap();
        assert_eq!(
            item.kind,
            ItemKind::Command {
                path: PathBuf::from("acpi"),
                args: vec![String::from("-b"), String::from("-i")]
            }
        );
    }
}
