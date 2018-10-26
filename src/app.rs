
use std::thread;
use std::time::Duration;
use std::fs::{File, OpenOptions};
use std::process::Command;
use std::io::{Read, Write};

use conf::Config;
use time::get_time;
use item::ItemKind;
use item::Mapper;

pub fn start(mut conf: Config) {
    // We would deamonize here if necessary

    loop {
        loop {
            let cur_time = get_time().sec;
            if let Some(c) = conf.items.iter().peekable().peek() {
                if c.next_time > cur_time {
                    break;
                }
            } else {
                break;
            }


            let mut item = conf.items.pop().unwrap();
            let clone = item.clone();
            item.next_time = cur_time + item.interval;
            conf.items.push(item);

            let mut shell = String::new();

            let mut output_folder = conf.general.output.clone();

            if let ItemKind::Shell(_) = clone.kind {
                shell = conf.general.shell.clone();
            }

            thread::spawn(move || {
                let mut result = String::new();
                match clone.kind {
                    ItemKind::File(ref path) => {
                        let mut f = match File::open(path) {
                            Ok(f) => f,
                            Err(e) => return error!("Could not open file: {}\n{}", path.display(), e),
                        };
                        match f.read_to_string(&mut result) {
                            Ok(_) => (),
                            Err(e) => return error!("Could read output from file: {},\n{}", path.display(), e),
                        }
                    }
                    ItemKind::Command(ref path, ref args) => {
                        let mut output = Command::new(path);
                        output.args(args);
                        for (k,v) in clone.env.iter() {
                            output.env(k, v);
                        }
                        let output = match output.output() {
                            Ok(f) => f,
                            Err(e) => return error!("Could not run command: {}\n{}", path.display(), e)
                        };
                        result = match String::from_utf8(output.stdout) {
                            Ok(r) => r,
                            Err(e) => return error!("Could not read output from command: {}\n{}", path.display(), e)
                        }
                    }
                    ItemKind::Shell(ref command) => {
                        let mut output = Command::new(shell);
                        output.arg("-c");
                        output.arg(command);
                        for (k,v) in clone.env.iter() {
                            output.env(k, v);
                        }
                        let output = match output.output() {
                            Ok(f) => f,
                            Err(e) => return error!("Could not run shell command: {}\n{}", command, e)
                        };
                        result = match String::from_utf8(output.stdout) {
                            Ok(r) => r,
                            Err(e) => return error!("Could not read output from shell command: {}\n{}", command, e)
                        }
                    }
                }

            debug!("{}={}", clone.key, result);
            output_folder.push(&clone.key);
            match OpenOptions::new().write(true).append(true).create(true).open(&output_folder)
                .and_then(|mut file| {
                    if clone.mappers.is_empty() {
                        file.write(&format!("{} {}", cur_time, &result).as_bytes()[..])
                    } else {
                        for (i, regex) in clone.mappers
                            .iter()
                            .map(|mapper| match mapper {
                                Mapper::Regex { regex } => regex,
                            })
                            .enumerate()
                        {
                            for (j, mtch) in regex
                                .find_iter(&result)
                                .enumerate()
                            {
                                let out = format!("{time} {mapper}.{nmatch} {text}",
                                                  time = cur_time,
                                                  mapper = i,
                                                  nmatch = j,
                                                  text = mtch.as_str());
                                file.write(out.as_bytes())?;
                            }
                        }

                        Ok(Default::default())
                    }
                })
                {
                    Ok(_) => (),
                    Err(e) => {
                        error!("Error creating file {}, {}", output_folder.display(), e)
                    }
                }

            });
        }
        if let Some(c) = conf.items.iter().peekable().peek() {
            thread::sleep(Duration::from_secs((c.next_time - get_time().sec) as u64));
        }
    }
}

