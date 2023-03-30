use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use influxdb::{self, InfluxDbWriteable};
use log::{debug, error, warn};
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;

use crate::conf::OutputKind;
use crate::item::ItemResult;

#[async_trait]
pub trait AKOutput {
    fn prepare(&self) -> Result<()>;
    async fn start(self, mut receiver: broadcast::Receiver<ItemResult>);
}

#[derive(Clone)]
pub enum Output {
    File(FileOutput),
    InfluxDB(InfluxDBOutput),
}

#[async_trait]
impl AKOutput for Output {
    fn prepare(&self) -> Result<()> {
        match self {
            Self::File(output) => output.prepare(),
            Self::InfluxDB(output) => output.prepare(),
        }
    }
    async fn start(self, receiver: broadcast::Receiver<ItemResult>) {
        match self {
            Self::File(output) => output.start(receiver).await,
            Self::InfluxDB(output) => output.start(receiver).await,
        }
    }
}

impl From<OutputKind> for Output {
    fn from(ok: OutputKind) -> Self {
        match ok {
            OutputKind::File {
                base_path,
                always_write_raw,
            } => Output::File(FileOutput {
                base_path,
                always_write_raw,
            }),
            OutputKind::InfluxDB {
                url,
                database,
                auth,
                use_raw_as_fallback,
                always_write_raw,
            } => {
                let client = auth
                    .as_ref()
                    .map(|crate::conf::InfluxDBAuth { username, password }| {
                        influxdb::Client::new(url.clone(), database.clone())
                            .with_auth(username, password)
                    })
                    .unwrap_or_else(|| influxdb::Client::new(url, database));
                Output::InfluxDB(InfluxDBOutput {
                    use_raw_as_fallback,
                    always_write_raw,
                    client,
                })
            }
        }
    }
}

#[derive(Clone)]
pub struct FileOutput {
    base_path: PathBuf,
    always_write_raw: bool,
}

impl FileOutput {
    async fn open_file(&self, key: &str) -> Result<File> {
        let mut path = self.base_path.clone();
        path.push(key.replace('/', "_"));
        OpenOptions::new()
            .write(true)
            .append(true)
            .create(true)
            .open(&path)
            .await
            .map_err(anyhow::Error::from)
    }
    async fn write_raw_value(&self, key: &str, value: &str, time: &Duration) -> Result<()> {
        let mut file = self.open_file(key).await?;
        file.write_all(format!("{} {}\n", time.as_secs(), value).as_bytes())
            .await?;
        Ok(())
    }
    async fn write_value(&self, key: &str, value: f64, time: &Duration) -> Result<()> {
        let mut file = self.open_file(key).await?;
        file.write_all(format!("{} {}\n", time.as_secs(), value).as_bytes())
            .await?;
        Ok(())
    }
    async fn write_values(&self, values: &HashMap<String, f64>, time: &Duration) -> Result<()> {
        for (key, value) in values.iter() {
            self.write_value(key, *value, time).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl AKOutput for FileOutput {
    fn prepare(&self) -> Result<()> {
        std::fs::create_dir_all(self.base_path.clone()).map_err(anyhow::Error::from)
    }
    async fn start(self, mut receiver: broadcast::Receiver<ItemResult>) {
        debug!("FileOutput: Starting loop");
        loop {
            match receiver.recv().await {
                Err(recverr) => match recverr {
                    broadcast::error::RecvError::Closed => break,
                    broadcast::error::RecvError::Lagged(count) => {
                        warn!("FileOutput is lagging behind, {} results skipped", count)
                    }
                },
                Ok(itemresult) => {
                    debug!("FileOutput: Received result for item {}", itemresult.key);
                    debug!("FileOutput: values: {:#?}", itemresult.values);
                    if itemresult.values.is_empty() || self.always_write_raw {
                        if let Err(e) = self
                            .write_raw_value(
                                &format!("{}.raw", itemresult.key),
                                &itemresult.raw,
                                &itemresult.time,
                            )
                            .await
                        {
                            error!(
                                "FileOutput: Failed writing data for Item {}",
                                itemresult.key
                            );
                            error!("FileOutput: {}", e);
                        }
                    }
                    if !itemresult.values.is_empty() {
                        if let Err(e) = self
                            .write_values(&itemresult.values, &itemresult.time)
                            .await
                        {
                            error!(
                                "FileOutput: Failed writing data for Item {}",
                                itemresult.key
                            );
                            error!("FileOutput: {}", e);
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub struct InfluxDBOutput {
    use_raw_as_fallback: bool,
    always_write_raw: bool,
    client: influxdb::Client,
}

impl InfluxDBOutput {
    async fn write_raw_value(&self, key: &str, value: &str, time: &Duration) -> Result<()> {
        self.client
            .query(
                influxdb::Timestamp::Milliseconds(time.as_millis())
                    .into_query(key)
                    .add_field("value", value),
            )
            .await
            .map(|_| ())
            .map_err(anyhow::Error::from)
    }
    async fn write_values(&self, values: &HashMap<String, f64>, time: &Duration) -> Result<()> {
        self.client
            .query(
                values
                    .iter()
                    .map(|(key, value)| {
                        influxdb::Timestamp::Milliseconds(time.as_millis())
                            .into_query(key)
                            .add_field("value", value)
                    })
                    .collect::<Vec<influxdb::WriteQuery>>(),
            )
            .await
            .map(|_| ())
            .map_err(anyhow::Error::from)
    }
}

#[async_trait]
impl AKOutput for InfluxDBOutput {
    fn prepare(&self) -> Result<()> {
        Ok(())
    }
    async fn start(self, mut receiver: broadcast::Receiver<ItemResult>) {
        debug!("InfluxDBOutput: Starting loop");
        loop {
            match receiver.recv().await {
                Err(recverr) => match recverr {
                    broadcast::error::RecvError::Closed => break,
                    broadcast::error::RecvError::Lagged(count) => {
                        warn!(
                            "InfluxDBOutput is lagging behind, {} results skipped",
                            count
                        )
                    }
                },
                Ok(itemresult) => {
                    debug!(
                        "InfluxDBOutput: Received result for item {}",
                        itemresult.key
                    );
                    debug!("InfluxDBOutput: values: {:#?}", itemresult.values);
                    if itemresult.values.is_empty() && self.use_raw_as_fallback
                        || self.always_write_raw
                    {
                        if let Err(e) = self
                            .write_raw_value(
                                &format!("{}.raw", itemresult.key),
                                &itemresult.raw,
                                &itemresult.time,
                            )
                            .await
                        {
                            error!(
                                "InfluxDBOutput: Failed writing raw data for Item {}",
                                itemresult.key
                            );
                            error!("InfluxDBOutput: {}", e);
                        }
                    }
                    if !itemresult.values.is_empty() {
                        if let Err(e) = self
                            .write_values(&itemresult.values, &itemresult.time)
                            .await
                        {
                            error!(
                                "InfluxDBOutout: Failed writing data for Item {}",
                                itemresult.key
                            );
                            error!("InfluxDBOutput: {}", e)
                        }
                    }
                }
            }
        }
    }
}
