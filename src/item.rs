
use std::path::PathBuf;
use std::error::Error;
use std::collections::BTreeMap;

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
            ItemErrorKind::InvalidInterval      => "interval has to be bigger than 0 and smaller than MAX_INT64",
        }
    }
}

impl ItemError {
    pub fn new(key: String ,k: ItemErrorKind) -> ItemError {
        ItemError {
            key: key,
            kind: k,
        }
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
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd, Deserialize)]
#[serde(untagged)]
pub enum ItemKind {
    /// Read the file at the given location, useful on Linux for the /sys dir for example
    File(PathBuf),
    /// Path to an executable with a list of arguments to be given to the executable
    Command(PathBuf, Vec<String>),
    /// A string to be executed in a shell context
    Shell(String),
}

/// A single item, knowing when it is supposed to run next, what should be done and its key.
#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    #[serde(skip, default = "next_time_default")]
    pub next_time: i64,
    pub interval: i64,
    pub key: String,

    #[serde(skip, default = "BTreeMap::new")]
    pub env: BTreeMap<String, String>,

    #[serde(rename = "command")]
    pub kind: ItemKind,

    #[serde(default = "Vec::new")]
    pub mappers: Vec<Mapper>,
}

#[derive(Debug, Clone, Deserialize)]
pub enum Mapper {
    Regex {
        #[serde(with = "serde_regex")]
        regex: ::regex::Regex,
    },
    // Maybe later more?
}

fn next_time_default() -> i64 {
    0
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::collections::BTreeMap;

    use item::{Item,ItemKind};

    #[test]
    fn items_ordered_by_smallest_time_first() {
        let mut heap = Vec::new();
        heap.push(Item {
            next_time: 5,
            interval: 5,
            env: BTreeMap::new(),
            key: String::from("tests.one"),
            kind: ItemKind::File(PathBuf::from("/dev/null")),
            mappers: vec![],
        });
        heap.push(Item {
            next_time: 3,
            interval: 5,
            env: BTreeMap::new(),
            key: String::from("tests.two"),
            kind: ItemKind::File(PathBuf::from("/dev/null")),
            mappers: vec![],
        });

        if let Some(item) = heap.pop() {
            assert_eq!(item.key, "tests.two");
        } else {
            unreachable!();
        }
    }
}
