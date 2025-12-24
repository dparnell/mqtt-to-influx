use anyhow::{anyhow, Result};
use clap::Parser;
use evalexpr::{eval_with_context_mut, HashMapContext, Value, ContextWithMutableVariables};
use jsonpath_rust::JsonPathFinder;
use log::{debug, error, info};
use rumqttc::{AsyncClient, MqttOptions, QoS, Event, Packet};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::time::Duration;

#[derive(Debug, Deserialize, Clone)]
struct Config {
    mqtt_host: String,
    mqtt_port: u16,
    mqtt_topic: String,
    log_level: Option<String>,
    terminate_on_error: Option<bool>,
    influxdb: InfluxConfig,
    measurements: Vec<MeasurementConfig>,
}

#[derive(Debug, Deserialize, Clone)]
struct InfluxConfig {
    version: u8,
    url: String,
    bucket: String,
    org: Option<String>,
    token: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
struct MeasurementConfig {
    name: String,
    path: String,
    expression: Option<String>,
    tags: Option<HashMap<String, String>>,
}

enum InfluxClient {
    V1(influxdb::Client),
    V2(influxdb2::Client),
}

impl InfluxClient {
    fn new(config: &InfluxConfig) -> Self {
        match config.version {
            1 => {
                let client = influxdb::Client::new(&config.url, &config.bucket);
                let client = if let Some(token) = &config.token {
                    let parts: Vec<&str> = token.split(':').collect();
                    if parts.len() == 2 {
                        client.with_auth(parts[0], parts[1])
                    } else {
                        client
                    }
                } else {
                    client
                };
                InfluxClient::V1(client)
            }
            2 => {
                let client = influxdb2::Client::new(
                    &config.url,
                    config.org.as_deref().unwrap_or(""),
                    config.token.as_deref().unwrap_or(""),
                );
                InfluxClient::V2(client)
            }
            _ => panic!("Unsupported InfluxDB version: {}", config.version),
        }
    }

    async fn write(&self, measurement: &str, value: f64, bucket: &str, tags: &Option<HashMap<String, String>>) -> Result<()> {
        match self {
            InfluxClient::V1(client) => {
                let mut query = influxdb::WriteQuery::new(chrono::Utc::now().into(), measurement)
                    .add_field("value", value);
                if let Some(tags) = tags {
                    for (key, val) in tags {
                        query = query.add_tag(key.clone(), val.clone());
                    }
                }
                client.query(query).await.map_err(|e: influxdb::Error| anyhow!(e))?;
            }
            InfluxClient::V2(client) => {
                let mut builder = influxdb2::models::DataPoint::builder(measurement)
                    .field("value", value);
                if let Some(tags) = tags {
                    for (key, val) in tags {
                        builder = builder.tag(key, val);
                    }
                }
                let data_point = builder.build()?;
                client.write(bucket, tokio_stream::iter(vec![data_point])).await?;
            }
        }
        Ok(())
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let config_content = fs::read_to_string(&args.config)
        .map_err(|e| anyhow!("Failed to read config file {}: {}", args.config, e))?;
    let config: Config = toml::from_str(&config_content)?;

    let log_level = config.log_level.as_deref().unwrap_or("info");
    env_logger::init_from_env(env_logger::Env::default().default_filter_or(log_level));

    let influx_client = InfluxClient::new(&config.influxdb);

    let mut mqttoptions = MqttOptions::new("mqtt_to_influx_bridge", &config.mqtt_host, config.mqtt_port);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    client.subscribe(&config.mqtt_topic, QoS::AtLeastOnce).await?;

    info!("Connected to MQTT and subscribed to {}", config.mqtt_topic);

    let terminate_on_error = config.terminate_on_error.unwrap_or(false);

    loop {
        match eventloop.poll().await {
            Ok(Event::Incoming(Packet::Publish(publish))) => {
                if let Err(e) = process_message(&publish.payload, &config, &influx_client).await {
                    error!("Error processing message: {}", e);
                    if terminate_on_error {
                        return Err(e);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("Error in event loop: {}", e);
                if terminate_on_error {
                    return Err(e.into());
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

async fn process_message(payload: &[u8], config: &Config, influx_client: &InfluxClient) -> Result<()> {
    let payload_str = std::str::from_utf8(payload)?;
    let json: serde_json::Value = serde_json::from_str(payload_str)?;

    for m_config in &config.measurements {
        let finder = JsonPathFinder::from_str(&json.to_string(), &m_config.path)
            .map_err(|e| anyhow!("Invalid JSONPath {}: {}", m_config.path, e))?;
        
        let found = finder.find();
        
        if let Some(val) = found.as_array().and_then(|a| a.first()) {
            let mut float_val = if val.is_number() {
                val.as_f64().unwrap_or(0.0)
            } else if val.is_string() {
                val.as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0)
            } else {
                continue;
            };

            if let Some(expr) = &m_config.expression {
                let mut context = HashMapContext::new();
                context.set_value("value".into(), Value::Float(float_val))?;
                if let Ok(eval_res) = eval_with_context_mut(expr, &mut context) {
                    if let Ok(f) = eval_res.as_float() {
                        float_val = f;
                    } else if let Ok(i) = eval_res.as_int() {
                        float_val = i as f64;
                    }
                }
            }

            debug!("Writing measurement: {} = {}", m_config.name, float_val);
            influx_client.write(&m_config.name, float_val, &config.influxdb.bucket, &m_config.tags).await?;
        }
    }

    Ok(())
}
