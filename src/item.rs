
use std::path::PathBuf;
use std::error::Error;
use std::collections::BTreeMap;

use toml;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ItemErrorKind {
    MissingValueSection,
    MissingIntervalSection,
    ValueArrayInvalid,
    ValueTableMissingKey,
    InvalidValueType,
    InvalidShellType,
    InvalidPathType,
    MultipleSources,
    MissingKey,
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
            ItemErrorKind::MissingValueSection  => "missing 'command', 'shell' or 'file' key",
            ItemErrorKind::MissingIntervalSection   => "missing 'interval' key",
            ItemErrorKind::ValueArrayInvalid    => "specified an empty array as command",
            ItemErrorKind::ValueTableMissingKey => "specified a table with missing path and/or args",
            ItemErrorKind::InvalidValueType     => "invalid value type, you may only use tables, strings and arrays",
            ItemErrorKind::InvalidShellType
                | ItemErrorKind::InvalidPathType      => "invalid value type, you may only use a string",
            ItemErrorKind::MultipleSources      => "multiple sources given, you may only use command or file or shell",
            ItemErrorKind::MissingKey           => "missing key field",
            ItemErrorKind::InvalidInterval      => "interval has to be bigger than 0 and smaller than MAX_INT64",
        }
    }
}

impl ItemError {
    fn new(key: String ,k: ItemErrorKind) -> ItemError {
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
#[derive(Debug, Clone, Eq, PartialEq, PartialOrd)]
pub enum ItemKind {
    /// Read the file at the given location, useful on Linux for the /sys dir for example
    File(PathBuf),
    /// Path to an executable with a list of arguments to be given to the executable
    Command(PathBuf, Vec<String>),
    /// A string to be executed in a shell context
    Shell(String),
}

/// A single item, knowing when it is supposed to run next, what should be done and its key.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Item {
    pub next_time: i64,
    pub interval: i64,
    pub key: String,
    pub env: BTreeMap<String, String>,
    pub kind: ItemKind,
}

impl Item {
    pub fn from_toml(table: &toml::Table) -> Result<Item, ItemError> {

        let key = match table.get("key") {
            Some(&toml::Value::String(ref s)) => s.clone(),
            _ => return Err(ItemError::new(String::from(""), ItemErrorKind::MissingKey))
        };

        let command = table.get("command")
            .ok_or_else(|| ItemError::new(key.clone(), ItemErrorKind::MissingValueSection))
            .and_then(|v| {
                let path : PathBuf;
                let args : Vec<String>;
                if let toml::Value::Table(ref v) = *v {
                    if let (Some(&toml::Value::String(ref s)), Some(&toml::Value::Array(ref a)))
                                                                 = (v.get("path"), v.get("args")) {
                        path = PathBuf::from(&s);
                        args = if a.iter().map(|x| x.as_str()).all(|x| x.is_some()) {
                            a.iter().map(|x| x.as_str()).map(|x| x.unwrap().into()).collect()
                        } else {
                            return Err(ItemError::new(key.clone(), ItemErrorKind::ValueArrayInvalid));
                        };

                        Ok(ItemKind::Command(path, args))
                    } else {
                        return Err(ItemError::new(key.clone(), ItemErrorKind::ValueTableMissingKey));
                    }
                } else if let toml::Value::Array(ref a) = *v {
                    if a.len() < 1 {
                        return Err(ItemError::new(key.clone(), ItemErrorKind::ValueArrayInvalid));
                    }
                    let mut iter = a.iter().map(|x| x.as_str());
                    if !iter.all(|x| x.is_some()) {
                        return Err(ItemError::new(key.clone(), ItemErrorKind::ValueArrayInvalid));
                    }
                    let mut strings = iter.map(|x| x.unwrap().into()).collect::<Vec<String>>();
                    path = PathBuf::from(strings.pop().unwrap());
                    args = strings;
                    Ok(ItemKind::Command(path, args))
                } else if let toml::Value::String(ref s) = *v {
                    path = PathBuf::from(s);
                    args = Vec::new();
                    Ok(ItemKind::Command(path, args))
                } else {
                    return Err(ItemError::new(key.clone(), ItemErrorKind::InvalidValueType));
                }
            });

        let shell = table.get("shell")
            .ok_or_else(|| ItemError::new(key.clone(), ItemErrorKind::MissingValueSection))
            .and_then(|v| if let toml::Value::String(ref s) = *v {
                Ok(ItemKind::Shell(s.clone()))
            } else {
                Err(ItemError::new(key.clone(), ItemErrorKind::InvalidShellType))
            });

        let path = table.get("file")
            .ok_or_else(|| ItemError::new(key.clone(), ItemErrorKind::MissingValueSection))
            .and_then(|v| if let toml::Value::String(ref s) = *v {
                Ok(ItemKind::File(PathBuf::from(s)))
            } else {
                Err(ItemError::new(key.clone(), ItemErrorKind::InvalidPathType))
            });

        let env = match table.get("env") {
            Some(&toml::Value::Table(ref x)) => x.iter()
                .map(|(k, v)| (k.clone(), v.as_str()))
                .filter(|&(_, v)| v.is_some())
                .map(|(k,v)| (k, v.unwrap().into()))
                .collect::<BTreeMap<String, String>>(),

            _ => BTreeMap::new()
        };

        debug!("Got this env: {:#?}", env);

        let sources = vec![command, shell, path];

        {
            if sources.iter().all(|x| x.is_err()) {
                return Err(ItemError::new(key.clone(), ItemErrorKind::MissingValueSection));
            }

            if sources.iter().filter(|x| x.is_ok()).count() > 2 {
                return Err(ItemError::new(key.clone(), ItemErrorKind::MultipleSources));
            }
        }

        let kind = try!(sources.into_iter().filter(|x| x.is_ok()).next().unwrap());

        let time = match table.get("interval") {
            Some(&toml::Value::Integer(x)) if x <= 0 => return Err(ItemError {
                key: key.clone(),
                kind: ItemErrorKind::InvalidInterval,
            }),
            Some(&toml::Value::Integer(x)) => x,
            _ => return Err(ItemError {
                key: key.clone(),
                kind: ItemErrorKind::MissingIntervalSection,
            })
        };

        Ok(Item {
            next_time: 0,
            interval: time,
            key: key,
            kind: kind,
            env: env,
        })
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<::std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> ::std::cmp::Ordering {
        if self.next_time < other.next_time {
            return ::std::cmp::Ordering::Greater
        } else {
            return ::std::cmp::Ordering::Less
        }
        ::std::cmp::Ordering::Equal
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::collections::BinaryHeap;
    use std::collections::BTreeMap;

    use item::{Item,ItemKind};

    #[test]
    fn items_ordered_by_smallest_time_first() {
        let mut heap = BinaryHeap::new();
        heap.push(Item {
            next_time: 5,
            interval: 5,
            env: BTreeMap::new(),
            key: String::from("tests.one"),
            kind: ItemKind::File(PathBuf::from("/dev/null")),
        });
        heap.push(Item {
            next_time: 3,
            interval: 5,
            env: BTreeMap::new(),
            key: String::from("tests.two"),
            kind: ItemKind::File(PathBuf::from("/dev/null")),
        });

        if let Some(item) = heap.pop() {
            assert_eq!(item.key, "tests.two");
        } else {
            unreachable!();
        }
    }
}
