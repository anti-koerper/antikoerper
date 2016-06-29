
use std::thread;
use std::time::Instant;
use std::fs::{File, OpenOptions};
use std::process::Command;
use std::io::{Read, Write};

use conf::Config;
use item::ItemKind;

pub fn start(mut conf: Config) {
    // We would deamonize here if necessary

    loop {
        loop {
            let cur_time = Instant::now();
            if let Some(c) = conf.items.peek() {
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

            let mut output_folder = conf.general.output.clone();

            let shell = if let ItemKind::Shell(_) = clone.kind {
                conf.general.shell.clone()
            } else {
                 String::new()
            };

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
                        for (k,v) in clone.env {
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
                        for (k,v) in clone.env {
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
                output_folder.push(clone.key);
                match OpenOptions::new().write(true).append(true).create(true).open(&output_folder)
                    .and_then(|mut file| {
                        file.write(&format!("{:?} {}", cur_time, &result).as_bytes()[..])
                    })
                    {
                        Ok(_) => (),
                        Err(e) => {
                            error!("Error creating file {}, {}", output_folder.display(), e)
                        }
                    }
            });
        }
        if let Some(c) = conf.items.peek() {
            let cur_time = Instant::now();
            if cur_time > c.next_time {
                thread::sleep(c.next_time.duration_since(cur_time));
            }
        }
    }
}

