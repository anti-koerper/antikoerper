//! Main application code of antikoerper

use tokio::task::JoinHandle;

use anyhow::Result;
use log::{debug, info, warn};
use tokio::sync::broadcast;

use crate::conf::{Config, General};
use crate::item::Item;
use crate::output::{AKOutput, Output};

pub struct App {
    general: General,
    items: Vec<Item>,
    outputs: Vec<Output>,
}

impl App {
    pub async fn start(&self) -> Result<()> {
        info!("Starting up antikoerper!");
        let (sender, _receiver) = broadcast::channel(100);
        let mut join_handles: Vec<JoinHandle<_>> = Vec::new();
        for item in &self.items {
            debug!("spawning item task {}", item.key);
            let s = sender.clone();
            let shell = self.general.shell.clone();
            let item = item.clone();
            join_handles.push(tokio::spawn(item.start(shell, s)));
        }
        for output in &self.outputs {
            debug!("spawning output tasks");
            output.prepare()?;
            let r = sender.subscribe();
            let op = output.clone();
            join_handles.push(tokio::spawn(op.start(r)));
        }
        for jh in join_handles {
            if let Err(e) = jh.await {
                warn!("Waiting on a thread failed");
                warn!("{}", e);
            }
        }
        debug!("all tasks have rejoined. Exiting.");
        Ok(())
    }
}

impl From<Config> for App {
    fn from(config: Config) -> Self {
        App {
            general: config.general,
            items: config.items,
            outputs: config
                .output
                .into_iter()
                .map(|ok| Output::from(ok))
                .collect(),
        }
    }
}
