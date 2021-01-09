use std::collections::BTreeMap;
use std::collections::HashMap;
use std::f64;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

use tokio::runtime::Runtime;

use crate::conf::Config;
use crate::item::{DigestKind, Item, ItemKind};
use crate::output::{AKOutput, OutputKind};

/// Create and starts the tokio runtime, adding one forever repeating task for
/// every configured item.
pub fn start(conf: Config) {
    // panics when failing to construct Runtime
    let mut runtime = Runtime::new().unwrap();
    let mut join_handles = vec![];

    for item in conf.items {
        // copy shell and outputs, as those are needed in every thread
        // item is only one per thread, and can be moved
        let shell = conf.general.shell.clone();
        let outputs = conf.output.clone();
        join_handles.push(runtime.spawn(item_worker(item, shell, outputs)));
    }
    runtime.block_on(async {
        for jh in join_handles {
            if let Err(e) = jh.await {
                error!("Failure joining an item_worker thread: {}", e);
            }
        }
    });
}

async fn item_worker(item: Item, shell: String, outputs: Vec<OutputKind>) {
    let mut interval = tokio::time::interval(Duration::from_secs(item.interval as u64));
    loop {
        interval.tick().await;

        //
        // generate/query for data as configured
        //
        let mut raw_result = String::new();
        match item.kind {
            ItemKind::File { ref path } => {
                let mut f = match File::open(path) {
                    Ok(file) => file,
                    Err(e) => {
                        error!("Could not open file: {}\n{}", path.display(), e);
                        continue;
                    }
                };
                match f.read_to_string(&mut raw_result) {
                    Ok(_) => {}
                    Err(e) => {
                        error!(
                            "Could not read output from file: {},\n{}",
                            path.display(),
                            e
                        );
                        continue;
                    }
                };
            }
            ItemKind::Command { ref path, ref args } => {
                raw_result = match run_cmd_capture_output(path, args, &item.env) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
            }
            ItemKind::Shell { ref script } => {
                raw_result = match run_cmd_capture_output(
                    &PathBuf::from(&shell),
                    &vec![String::from("-c"), script.clone()],
                    &item.env,
                ) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
            }
        }
        let raw_result = raw_result.trim();
        debug!("{}={}", item.key, raw_result);

        //
        // digest raw data as configured
        //
        let mut values: HashMap<String, f64> = HashMap::new();

        match item.digest {
            // no digest - use the fallback method
            // errors when trying to parse a float from the given raw value are logged as info
            // having not-parsable output is a valid use-case for antikoerper
            DigestKind::Raw => {
                let _ = raw_result
                    .parse::<f64>()
                    .map(|v| values.insert(format!("{}.parsed", &item.key).into(), v))
                    .map_err(|_| info!("Value could not be parsed as f64: {}", raw_result));
            }

            // digest using regexes, and write the extracted values
            DigestKind::Regex { ref regex } => {
                if let Some(captures) = regex.captures(raw_result) {
                    for cn in regex.capture_names() {
                        if let Some(named_group) = cn {
                            let value = captures[named_group].parse::<f64>().unwrap_or(f64::NAN);
                            values.insert(format!("{}.{}", &item.key, &named_group).into(), value);
                        }
                    }
                } else {
                    warn!(
                        "Provided regex did not match the output: {}\n{}",
                        regex, raw_result
                    );
                }
            }
        }

        //
        // write data to all configured outputs
        //
        let cur_time = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
            Ok(n) => n,
            Err(_) => panic!("SystemTime before UNIX EPOCH!"),
        };

        for outp in &outputs {
            if values.len() == 0 {
                let _ = outp
                    .write_raw_value_as_fallback(
                        &format!("{}.raw", &item.key),
                        cur_time,
                        raw_result,
                    )
                    .map_err(|e| error!("Failure writing to output: {}", e));
            } else {
                let _ = outp
                    .write_raw_value(&format!("{}.raw", &item.key), cur_time, raw_result)
                    .map_err(|e| error!("Failure writing to output: {}", e));

                for (k, v) in values.iter() {
                    let _ = outp
                        .write_value(&k, cur_time, *v)
                        .map_err(|e| error!("Failure writing to output: {}", e));
                }
            }
        }
    }
}

/// runs a command with the specified args and env, and returns
/// its stdout as a String
fn run_cmd_capture_output(
    cmd: &PathBuf,
    args: &[String],
    env: &BTreeMap<String, String>,
) -> Result<String, ()> {
    let mut command = Command::new(cmd);
    command.args(args);
    for (k, v) in env.iter() {
        command.env(k, v);
    }

    if let Ok(output) = command.output().map_err(|e| {
        error!("Could not run command: {}\n{}", cmd.display(), e);
        ()
    }) {
        match String::from_utf8(output.stdout) {
            Ok(s) => {
                return Ok(s);
            }
            Err(e) => {
                error!(
                    "Could not read output from command: {}\n{}",
                    cmd.display(),
                    e
                );
                return Err(());
            }
        }
    }
    Err(())
}
