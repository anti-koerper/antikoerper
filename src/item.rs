use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::sync::broadcast;

/// A single item, knowing when it is supposed to run next, what should be done and its key.
#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    pub interval: u64,
    pub key: String,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(rename = "input")]
    pub kind: ItemKind,
    #[serde(default)]
    pub digest: DigestKind,
}

impl Item {
    pub async fn start(self: Self, shell: String, sender: broadcast::Sender<ItemResult>) {
        debug!("item {}: starting loop", self.key);
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(self.interval));
        loop {
            interval.tick().await;
            match self.kind.produce_result(&shell, &self.env).await {
                Err(e) => {
                    error!("Item {} failed to produce a result", self.key);
                    error!("{}", e);
                }
                Ok(r) => {
                    if let Err(e) = sender.send(self.digest.digest(&r, &self.key)) {
                        error!("Result of Item {} could not be send via channel", self.key);
                        error!("{}", e);
                    }
                }
            }
        }
    }
}

/// The different kinds of items one can use
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ItemKind {
    /// Read the file at the given location, useful on Linux for the /sys or /proc dir for example
    File { path: PathBuf },
    /// Path to an executable with a list of arguments to be given to the executable
    Command {
        path: PathBuf,
        #[serde(default)]
        args: Vec<String>,
    },
    /// A string to be executed as a shell script
    Shell { script: String },
}

impl ItemKind {
    /// Generate a single result (raw, String)
    pub async fn produce_result(
        &self,
        shell: &str,
        env: &BTreeMap<String, String>,
    ) -> Result<String> {
        match &self {
            ItemKind::File { ref path } => {
                let mut file = tokio::fs::File::open(path)
                    .await
                    .with_context(|| format!("Failed to open file {}", path.display()))?;
                let mut buffer = String::new();
                file.read_to_string(&mut buffer)
                    .await
                    .with_context(|| format!("Failed to read from file {}", path.display()))?;
                Ok(buffer)
            }
            ItemKind::Command { path, args } => {
                run_cmd_capture_output(&path, args.as_slice(), env).await
            }
            ItemKind::Shell { script } => {
                run_cmd_capture_output(
                    &PathBuf::from(shell),
                    &["-c".into(), script.to_owned()],
                    env,
                )
                .await
            }
        }
    }
}

/// Wrapper around tokio::process::Command, which only returns stdout.
/// exitcode, stderr are ignored.
async fn run_cmd_capture_output(
    path: &PathBuf,
    args: &[String],
    env: &BTreeMap<String, String>,
) -> Result<String> {
    tokio::process::Command::new(path)
        .args(args)
        .envs(env.clone())
        .output()
        .await
        .with_context(|| format!("Failed running command {} {:#?}", path.display(), args))
        .and_then(|output| {
            String::from_utf8(output.stdout).with_context(|| {
                format!(
                    "Failed parsing utf8 from output of command {} {:#?}",
                    path.display(),
                    args
                )
            })
        })
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DigestKind {
    Regex {
        #[serde(with = "serde_regex")]
        regex: ::regex::Regex,
    },
    #[default]
    #[serde(rename = "none")]
    Raw,
    /// Parse the output of a monitoring plugin
    /// For infomation about such output, see
    /// https://www.monitoring-plugins.org/doc/guidelines.html#THRESHOLDFORMAT
    /// and https://www.monitoring-plugins.org/doc/guidelines.html#AEN201
    /// This here does not support the full standard; most notably, it supports
    /// only single lines of output, and does not really parse warning- and
    /// critical-ranges, those are only used if they are parsable as f64's
    #[serde(rename = "monitoring-plugin")]
    MonitoringPlugin {
        #[serde(skip, default = "monitoring_plugin_regex")]
        regex: (::regex::Regex, ::regex::Regex),
    }, // Maybe later more?
}

fn monitoring_plugin_regex() -> (::regex::Regex, ::regex::Regex) {
    (
        // Output of monitoring plugins is semi-standardized.
        // It's usually a human-readable message, then a pipe |, and then
        // performance metrics.
        // At least, for single lines of output. In theory, there could be
        // multiple lines with this format.
        ::regex::Regex::new(
            r"((?P<status>OK|WARNING|CRITICAL|UNKNOWN)[^\|]*)?\|(?P<performance>.*)$",
        )
        .unwrap(),
        // performance metrics in monitoring plugins are:
        //   * a label, which must not containt =
        //   * =
        //   * a value, numeric, with an optional unit (time: s, ms, ns, us; size: B, KB, MB, GB, TB; percentage: %; count: c)
        //   * optional, a warning range
        //   * optional, a critical range
        //   * optional, a min value
        //   * optional, a max value
        // This regex already look pretty bad, but it doesn't even "properly"
        // parse the warn/crit-ranges.
        ::regex::Regex::new(
            r"(?P<label>[^\s=][^=]*)=(?P<value>[-\.\d]+)(?P<unit>s|ms|ns|us|B|KB|MB|GB|TB|%|c)?(;(?P<warn>[@-~\.\d]+))?(;(?P<crit>[@-~\.\d]+))?(;(?P<min>[-\.\d]+))?(;(?P<max>[-\.\d]+))?;?"
        ).unwrap(),
    )
}

impl DigestKind {
    /// If configured, parse a raw result (String) into one or more f64 values,
    /// and produce an ItemResult
    pub fn digest(&self, result: &str, itemkey: &str) -> ItemResult {
        let result = result.trim();
        let mut values = HashMap::<String, f64>::new();
        match self {
            DigestKind::Raw => match result.parse::<f64>() {
                Ok(f) => {
                    values.insert(format!("{}.parsed", itemkey), f);
                }
                Err(_) => info!("Value could not be parsed as f64: {}", result),
            },

            // digest using regexes, and write the extracted values
            DigestKind::Regex { ref regex } => {
                debug!("item {}: regex digest", itemkey);
                if let Some(captures) = regex.captures(result) {
                    debug!("regex captures: {:#?}", captures);
                    for cn in regex.capture_names().flatten() {
                        let value = captures[cn].parse::<f64>().unwrap_or(f64::NAN);
                        debug!(
                            "item {}: parsed value {} for capture group {}",
                            itemkey, value, cn
                        );
                        values.insert(format!("{}.{}", itemkey, &cn), value);
                    }
                } else {
                    warn!(
                        "Provided regex did not match the output: {}\n{}",
                        regex, result
                    );
                }
            }
            DigestKind::MonitoringPlugin {
                regex: (output_regex, performance_regex),
            } => {
                debug!("item {}: monitoring-plugin-digest", itemkey);
                debug!("item {}: {}", itemkey, result);
                if let Some(output_matches) = output_regex.captures(result) {
                    debug!("monitoring plugin matches: {:#?}", output_matches);
                    output_matches.name("status").and_then(|status| {
                        let status_val = match status.as_str() {
                            "OK" => 0f64,
                            "WARNING" => 1f64,
                            "CRITICAL" => 2f64,
                            "UNKNOWN" => 3f64,
                            _ => return None,
                        };
                        values.insert(format!("{}.status", itemkey), status_val)
                    });
                    if let Some(perf_metrics) = output_matches.name("performance") {
                        debug!(
                            "monitoring plugin performance metric matches: {:#?}",
                            perf_metrics
                        );
                        for capture in performance_regex.captures_iter(perf_metrics.as_str()) {
                            let label = match capture.name("label") {
                                Some(l) => l.as_str(),
                                None => continue,
                            };
                            let mut value = capture
                                .name("value")
                                .and_then(|v| v.as_str().parse::<f64>().ok())
                                .unwrap_or(f64::NAN);
                            let value_factor = match capture.name("unit").map(|u| u.as_str()) {
                                Some("KB") => 1024f64,
                                Some("MB") => 1024f64.powi(2),
                                Some("GB") => 1024f64.powi(3),
                                Some("TB") => 1024f64.powi(4),
                                _ => 1f64,
                            };
                            value = value * value_factor;
                            values.insert(format!("{}.{}", itemkey, label), value);
                            for extra in ["warn", "crit", "min", "max"] {
                                capture
                                    .name(extra)
                                    .and_then(|v| v.as_str().parse::<f64>().ok())
                                    .and_then(|v| {
                                        values.insert(
                                            format!("{}.{}.{}", itemkey, label, extra),
                                            v * value_factor,
                                        )
                                    });
                            }
                        }
                    }
                }
            }
        };
        ItemResult {
            time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("SystemTime before UNIX EPOCH!"),
            key: itemkey.into(),
            raw: String::from(result),
            values,
        }
    }
}

#[derive(Clone)]
pub struct ItemResult {
    pub time: Duration,
    pub key: String,
    pub raw: String,
    pub values: HashMap<String, f64>,
}

#[cfg(test)]
mod tests {
    use crate::item::monitoring_plugin_regex;

    #[test]
    fn monitoring_plugin_regex_match() {
        let (output_rx, perf_rx) = monitoring_plugin_regex();
        let check_load = r"LOAD OK - load average: 0.31, 0.37, 0.29|load1=0.310;10.000;15.000;0; load5=0.370;5.000;6.000;0; load15=0.290;3.000;4.000;0;";
        assert!(output_rx.is_match(check_load));
        let captures = output_rx.captures(check_load).unwrap();
        assert_eq!(
            captures.name("status").and_then(|s| Some(s.as_str())),
            Some("OK")
        );
        assert!(captures.name("performance").is_some());
        let perf = captures.name("performance").unwrap().as_str();
        assert_eq!(
            perf,
            r"load1=0.310;10.000;15.000;0; load5=0.370;5.000;6.000;0; load15=0.290;3.000;4.000;0;"
        );
        let mut ci = perf_rx.captures_iter(perf);

        let capture = ci.next();
        assert!(capture.is_some());
        let capture = capture.unwrap();
        assert_eq!(capture.name("label").unwrap().as_str(), "load1");
        assert_eq!(capture.name("value").unwrap().as_str(), "0.310");
        assert!(capture.name("unit").is_none());
        assert_eq!(capture.name("warn").unwrap().as_str(), "10.000");
        assert_eq!(capture.name("crit").unwrap().as_str(), "15.000");
        assert_eq!(capture.name("min").unwrap().as_str(), "0");
        assert!(capture.name("max").is_none());

        let capture = ci.next();
        assert!(capture.is_some());
        let capture = capture.unwrap();
        assert_eq!(capture.name("label").unwrap().as_str(), "load5");
        assert_eq!(capture.name("value").unwrap().as_str(), "0.370");
        assert!(capture.name("unit").is_none());
        assert_eq!(capture.name("warn").unwrap().as_str(), "5.000");
        assert_eq!(capture.name("crit").unwrap().as_str(), "6.000");
        assert_eq!(capture.name("min").unwrap().as_str(), "0");
        assert!(capture.name("max").is_none());

        let capture = ci.next();
        assert!(capture.is_some());
        let capture = capture.unwrap();
        assert_eq!(capture.name("label").unwrap().as_str(), "load15");
        assert_eq!(capture.name("value").unwrap().as_str(), "0.290");
        assert!(capture.name("unit").is_none());
        assert_eq!(capture.name("warn").unwrap().as_str(), "3.000");
        assert_eq!(capture.name("crit").unwrap().as_str(), "4.000");
        assert_eq!(capture.name("min").unwrap().as_str(), "0");
        assert!(capture.name("max").is_none());

        let capture = ci.next();
        assert!(capture.is_none());
    }
}
