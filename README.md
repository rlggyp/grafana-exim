# grafana-exim

A simple tool for exporting and importing Grafana folders, dashboards, and datasources.

## Features

- Export and import folders, dashboards, and datasources between Grafana instances  
- Automatically cleans up dashboard metadata and handles folder associations  
- Configurable using YAML for easier management  
- Structured logging via `log4rs`  

## Configuration

Example configuration file:

```yaml
grafana:
  src:
    host: http://192.168.1.1:3000
    api_key: glsa_ABCDEFGHIJK1234567890abcdefghijk_lmnopqrs

  dst:
    host: http://192.168.2.1:3000
    api_key: glsa_LMNOPQRSTUV1234567890lmnopqrstuv_abcdefgh

log_config: 
  filepath: log4rs.yaml # full path or relative path
````

### Authentication

This tool authenticates requests to Grafana using **Service Account Tokens**.

How to create a Service Account Token:

1. Log in to your Grafana instance with admin privileges.
2. Navigate to **Administration** in the left-hand menu.
3. Select **Users and access**, then go to the **Service Accounts** tab.
4. Create a new service account and generate its token.
5. Copy the token securely — you will use it as the API key in your YAML config file.

The tool will send the token in the `Authorization` header:

```
Authorization: Bearer <YOUR_SERVICE_ACCOUNT_TOKEN>
```

For more information, see the [Grafana Service Account Token documentation](https://grafana.com/docs/grafana/latest/developers/http_api/authentication/#service-account-token).

## Logging Configuration

Logging is managed through [`log4rs`](https://docs.rs/log4rs). For most use cases, writing logs to stdout is sufficient. Below is an example `log4rs.yaml` configuration:

```yaml
refresh_rate: 30 seconds

appenders:
  stdout:
    kind: console
    encoder:
      pattern: "{d(%Y-%m-%d %H:%M:%S)} [{l}] {t} - {m}{n}"

root:
  level: info
  appenders:
    - stdout
```

If needed, file-based logging can also be enabled by extending the `appenders` section. See the log4rs documentation for details.

## Usage

Run the following commands to export and import data between Grafana instances:

```bash
cargo run
```

## References

This project makes use of the following Grafana HTTP APIs:

* [Folder & Dashboard Search API](https://grafana.com/docs/grafana/latest/developers/http_api/folder_dashboard_search/)
* [Folder API](https://grafana.com/docs/grafana/latest/developers/http_api/folder/)
* [Dashboard API](https://grafana.com/docs/grafana/latest/developers/http_api/dashboard/)
* [Data Source API](https://grafana.com/docs/grafana/latest/developers/http_api/data_source/)

## License

This project is licensed under the [MIT License](LICENSE). See the LICENSE file for details.
