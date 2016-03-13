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

use std::fs::File;
use std::path::PathBuf;

use clap::{Arg, App};

mod conf;
mod item;

fn main() {

    let xdg_dirs = xdg::BaseDirectories::with_prefix("antikoerper").unwrap();

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
                    .arg(Arg::with_name("v")
                         .short("v")
                         .multiple(true)
                         .help("Sets the level of verbosity"))
                    .get_matches();

    let config_path = matches.value_of("config").and_then(|s| {
        Some(PathBuf::from(s))
    }).or_else(|| {
        xdg_dirs.find_config_file("config.toml")
    });

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

    let level = match matches.occurrences_of("v") {
        0 => log::LogLevelFilter::Off,
        1 => log::LogLevelFilter::Warn,
        2 => log::LogLevelFilter::Debug,
        3 | _ => log::LogLevelFilter::Trace,
    };

    env_logger::LogBuilder::new().filter(None, level).init().unwrap();

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

    let config = match conf::load(&mut config_file) {
        Ok(c) => c,
        Err(e) => return println!("Error at loading config file ({}): \n{}",
                                  config_path.display() , e),
    };

}
