use std::sync::Arc;
use std::time::Duration;

use influxdb::{Client, Query, Timestamp};
use tokio;

use crate::item::Item;
use crate::output::{error::*, AKOutput};

#[derive(PartialEq, Eq, Debug, Clone, Deserialize)]
pub struct InfluxAuth {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InfluxOutput {
    #[serde(default = "influx_url_default")]
    pub url: String,
    #[serde(default = "influx_database_default")]
    pub database: String,
    #[serde(flatten)]
    pub auth: Option<InfluxAuth>,
    #[serde(default = "influx_raw_as_fallback_default")]
    pub use_raw_as_fallback: bool,
    #[serde(default = "influx_always_raw_default")]
    pub always_write_raw: bool,
    #[serde(skip, default)]
    pub client: Option<Arc<Client>>,
}

fn influx_database_default() -> String {
    String::from("antikoerper")
}

fn influx_url_default() -> String {
    String::from("http://localhost:8086")
}

fn influx_raw_as_fallback_default() -> bool {
    false
}

fn influx_always_raw_default() -> bool {
    false
}

impl PartialEq for InfluxOutput {
    fn eq(&self, other: &InfluxOutput) -> bool {
        self.database == other.database
            && self.url == other.url
            && self.auth == other.auth
            && self.use_raw_as_fallback == other.use_raw_as_fallback
            && self.always_write_raw == other.always_write_raw
    }
}

impl Eq for InfluxOutput {}

impl AKOutput for InfluxOutput {
    fn prepare(&self, _items: &[Item]) -> Result<Self, OutputError> {
        trace!("running prepare for InfluxOutput");
        if let Some(auth) = &self.auth {
            Ok(Self {
                url: self.url.clone(),
                database: self.database.clone(),
                auth: self.auth.clone(),
                use_raw_as_fallback: self.use_raw_as_fallback,
                always_write_raw: self.always_write_raw,
                client: Some(
                    Arc::new(
                        Client::new(self.url.clone(), self.database.clone())
                            .with_auth(auth.username.clone(), auth.password.clone()),
                    )
                ),
            })
        } else {
            Ok(Self {
                url: self.url.clone(),
                database: self.database.clone(),
                auth: self.auth.clone(),
                use_raw_as_fallback: self.use_raw_as_fallback,
                always_write_raw: self.always_write_raw,
                client: Some(
                    Arc::new(
                        Client::new(self.url.clone(), self.database.clone())
                    )
                ),
            })
        }
    }

    fn write_value(&self, key: &str, time: Duration, value: f64) -> Result<(), OutputError> {
        if let Some(client) = &self.client {
            let c = client.clone();
            let lkey = String::from(key);
            tokio::spawn(async move {
                if let Err(e) = c
                    .query(
                        &Query::write_query(
                            Timestamp::Milliseconds(time.as_millis() as usize),
                            lkey,
                        )
                        .add_field("value", value),
                    )
                    .await
                {
                    error!("failed to write to influxdb backend: {}", e);
                }
            });
            Ok(())
        } else {
            Err(OutputError {
                kind: OutputErrorKind::WriteError(
                    "InfluxOutput.write_value: client is null".into(),
                ),
                cause: None,
            })
        }
    }

    fn write_raw_value_as_fallback(
        &self,
        key: &str,
        time: Duration,
        value: &str,
    ) -> Result<(), OutputError> {
        if self.use_raw_as_fallback {
            if let Some(client) = &self.client {
                let c = client.clone();
                let lkey = String::from(key);
                let lval = String::from(value);
                tokio::spawn(async move {
                    if let Err(e) = c
                        .query(
                            &Query::write_query(
                                Timestamp::Milliseconds(time.as_millis() as usize),
                                lkey,
                            )
                            .add_field("value", lval),
                        )
                        .await
                    {
                        error!("failed to write to influxdb backend: {}", e);
                    }
                });
                Ok(())
            } else {
                Err(OutputError {
                    kind: OutputErrorKind::WriteError(
                        "InfluxOutput.write_value: client is null".into(),
                    ),
                    cause: None,
                })
            }
        } else {
            Ok(())
        }
    }

    fn write_raw_value(
        &self,
        key: &str,
        time: Duration,
        value: &str,
    ) -> Result<(), OutputError> {
        if self.always_write_raw {
            if let Some(client) = &self.client {
                let c = client.clone();
                let lkey = String::from(key);
                let lval = String::from(value);
                tokio::spawn(async move {
                    if let Err(e) = c
                        .query(
                            &Query::write_query(
                                Timestamp::Milliseconds(time.as_millis() as usize),
                                lkey,
                            )
                            .add_field("value", lval),
                        )
                        .await
                    {
                        error!("failed to write to influxdb backend: {}", e);
                    }
                });
                Ok(())
            } else {
                Err(OutputError {
                    kind: OutputErrorKind::WriteError(
                        "InfluxOutput.write_value: client is null".into(),
                    ),
                    cause: None,
                })
            }
        } else {
            Ok(())
        }
    }

    fn clean_up(&self) -> Result<(), OutputError> {
        Ok(())
    }
}
