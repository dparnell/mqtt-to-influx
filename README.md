# MQTT to InfluxDB Bridge

A lightweight Rust tool that subscribes to an MQTT topic, extracts data points from JSON payloads using JSONPath, optionally transforms them with expressions, and publishes the results to InfluxDB (v1 or v2).

## Features

- **MQTT Integration**: Connects to an MQTT broker and subscribes to a specific topic.
- **JSONPath Extraction**: Flexible data extraction from nested JSON structures.
- **Expression Evaluation**: Transform extracted values using mathematical expressions.
- **InfluxDB Support**: Supports both InfluxDB v1 (Database/Retention Policy) and InfluxDB v2 (Bucket/Org/Token).
- **Custom Tags**: Add static tags to your measurements for better filtering and grouping in InfluxDB.
- **Configurable Logging**: Set the log level (debug, info, warn, error) via configuration.
- **Optional Termination**: Optionally terminate the program if an error occurs during message processing or in the MQTT event loop.

## Installation

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (latest stable version)

### Build

```bash
git clone <repository-url>
cd mqtt-to-influx
cargo build --release
```

The binary will be available at `target/release/mqtt-to-influx`.

## Usage

1. Create a `config.toml` file in the same directory as the binary (see [Configuration](#configuration) below).
2. Run the bridge:

```bash
./mqtt-to-influx
```

By default, the tool looks for `config.toml` in the current directory. You can specify a custom configuration file using the `-c` or `--config` option:

```bash
./mqtt-to-influx --config /path/to/your/config.toml
```

## Configuration

The tool expects a `config.toml` file with the following structure:

```toml
mqtt_host = "localhost"
mqtt_port = 1883
mqtt_topic = "sensors/data"
log_level = "info" # debug, info, warn, error
terminate_on_error = false # terminate if an error occurs

[influxdb]
version = 2 # 1 or 2
url = "http://localhost:8086"
bucket = "my_bucket" # Database name for v1
org = "my_org"       # Ignored for v1
token = "my_token"   # Use "username:password" or empty for v1

[[measurements]]
name = "temperature"
path = "$.sensors.temp"
expression = "value * 1.8 + 32" # Celsius to Fahrenheit
tags = { sensor_id = "living_room", building = "main" }

[[measurements]]
name = "humidity"
path = "$.sensors.hum"
```

### Configuration Details

- **`mqtt_host`**: Address of the MQTT broker.
- **`mqtt_port`**: Port of the MQTT broker (usually 1883).
- **`mqtt_topic`**: The topic to subscribe to. The bridge expects JSON payloads on this topic.
- **`terminate_on_error`**: (Optional) If set to `true`, the program will terminate if an error occurs during message processing or in the MQTT event loop. Defaults to `false`.
- **`influxdb.version`**: Set to `1` for InfluxDB 1.x or `2` for InfluxDB 2.x/Cloud.
- **`influxdb.token`**: 
    - For v2: Your API token.
    - For v1: `username:password` string, or leave empty if no auth is required.
- **`measurements`**: A list of data points to extract from each incoming MQTT message.
    - **`name`**: The measurement name in InfluxDB.
    - **`path`**: A JSONPath expression to find the value in the JSON payload.
    - **`expression`**: (Optional) A mathematical expression to transform the value. Use `value` as the placeholder for the extracted number.
    - **`tags`**: (Optional) A map of key-value pairs to be added as tags to the measurement.

## JSONPath and Expressions

### JSONPath
The bridge uses [JSONPath](https://goessner.net/articles/JsonPath/) to locate values.
Example payload:
```json
{
  "meters": {
    "load": {
      "agg_p_mw": 1500
    }
  }
}
```
Path: `$.meters.load.agg_p_mw` extracts `1500`.

### Expressions
Powered by [evalexpr](https://crates.io/crates/evalexpr), you can perform arithmetic on the extracted values.
Example: `value / 1000.0` to convert milliwatts to watts.

## License

mqtt-to-influx © 2025 by Daniel Parnell is licensed under CC BY 4.0. To view a copy of this license, visit [the Creative Commons By 4.0 website](https://creativecommons.org/licenses/by/4.0/)

