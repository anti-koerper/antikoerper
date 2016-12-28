#![deny(missing_docs,
        missing_debug_implementations, missing_copy_implementations,
        trivial_casts, trivial_numeric_casts,
        unsafe_code,
        unstable_features,
        unused_import_braces, unused_qualifications)]

//! Antikoerper is a simple and lightweight data aggregation and visualization tool

extern crate rustc_serialize;
extern crate toml;
extern crate clap;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate xdg;
extern crate time;

use std::fs::File;
use std::path::PathBuf;
use std::process;
use std::env;

use clap::{Arg, App};

mod conf;
mod item;
mod app;

fn main() {
    let matches = App::new("Antikörper")
                    .version(env!("CARGO_PKG_VERSION"))
                    .author("Neikos <neikos@neikos.email>")
                    .about("Lightweight data aggregation and visualization tool.")
                    .after_help("You can output logging information by using the RUST_LOG env var.")
                    .arg(Arg::with_name("config")
                         .short("c")
                         .long("config")
                         .value_name("FILE")
                         .help("Sets a custom config file")
                         .takes_value(true))
                    .arg(Arg::with_name("output")
                         .short("o")
                         .long("output")
                         .value_name("DIRECTORY")
                         .help("Set the output path")
                         .takes_value(true))
                    .arg(Arg::with_name("daemonize")
                         .short("d")
                         .long("daemonize")
                         .multiple(false)
                         .takes_value(false)
                         .help("Starts antikoerper in daemon mode"))
                    .arg(Arg::with_name("v")
                         .short("v")
                         .multiple(true)
                         .help("Sets the level of verbosity"))
                    .get_matches();

    trace!("Getting XDG Base directories");
    let xdg_dirs = xdg::BaseDirectories::with_prefix("antikoerper").unwrap();

    let level = match matches.occurrences_of("v") {
        0 => log::LogLevelFilter::Off,
        1 => log::LogLevelFilter::Warn,
        2 => log::LogLevelFilter::Debug,
        3 | _ => log::LogLevelFilter::Trace,
    };

    env_logger::LogBuilder::new().filter(None, level).init().unwrap();

    trace!("Matching for config value");
    let config_path = matches.value_of("config")
        .map(PathBuf::from)
        .or_else(|| xdg_dirs.find_config_file("config.toml"));

    trace!("Value is: {:#?}", config_path);

    let config_path = match config_path {
        Some(x) => x,
        None => {
            println!("Could not find config file, make sure to give one with the --config option.");
            println!("The default is XDG_CONFIG_HOME/antikoerper/config.toml");
            println!("");
            println!("Check out https://github.com/anti-koerper/antikoerper for details
on what should be in that file.");
            return;
        }
    };

    trace!("Matching for output value");
    let data_path = matches.value_of("output").map(PathBuf::from).unwrap_or(PathBuf::new());

    if matches.is_present("daemonize") {

        let mut child = process::Command::new(std::env::args().next().unwrap());
        let args = env::args().skip(1).filter(|a| a != "--daemonize" && a != "-d")
            .collect::<Vec<_>>();
        child.args(&args).stdin(process::Stdio::null()).stdout(process::Stdio::null()).stderr(process::Stdio::null());
        match child.spawn() {
            Ok(_) => debug!("Successfully daemonized"),
            Err(e) => debug!("Failed daemonizing the process {:#?}", e),
        }
        return;
    }

    info!("Config file used: {}", &config_path.display());

    let mut config_file = {
        let file = File::open(&config_path);
        match file {
            Ok(f) => f,
            Err(e) => {
                debug!("{}", e);
                println!("Could not open file '{}': {}", config_path.display(), e);
                return;
            }
        }
    };

    let config = match conf::load(&mut config_file, data_path) {
        Ok(c) => c,
        Err(e) => return println!("Error at loading config file ({}): \n{}",
                                  config_path.display() , e),
    };

    app::start(config);

}
