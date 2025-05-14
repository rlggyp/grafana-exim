# grafana-exim

A simple tool for exporting and importing Grafana dashboards.

## Features

- Export dashboards and folders from a Grafana source instance  
- Import dashboards and folders to a Grafana destination instance  
- Supports asynchronous processing for faster operations  
- Automatically cleans up dashboard metadata and handles folder associations  

## Environment Setup

Set up your Grafana credentials by creating a `.env` file or setting environment variables:

```env
GRAFANA_SRC_HOST=http://192.168.1.1:3000
GRAFANA_SRC_API_KEY=<your_service_account_token>

GRAFANA_DST_HOST=http://192.168.2.1:3000
GRAFANA_DST_API_KEY=<your_service_account_token>
```

Replace `<your_service_account_token>` with your actual Service Account tokens.

## Authentication

This tool authenticates requests to Grafana using **Service Account Tokens**.

### How to create a Service Account Token:

1. Log in to your Grafana instance with admin privileges.  
2. Navigate to **Administration** in the left-hand menu.  
3. Select **Users and access**, then go to the **Service Accounts** tab.  
4. Create a new service account and generate its token.  
5. Copy the token securely — you will use it as the API key in your `.env` file.  

Use this token in the `Authorization` header as follows in all API requests:

```
Authorization: Bearer <YOUR_SERVICE_ACCOUNT_TOKEN>
```

For more information, see the [Grafana Service Account Token documentation](https://grafana.com/docs/grafana/latest/developers/http_api/authentication/#service-account-token).

## Usage

Run the following commands to export dashboards and folders from the source Grafana instance, and import them into the destination instance:

```bash
cargo run -- export
cargo run -- import
```

## References

This project makes use of the following Grafana HTTP APIs:

- [Folder & Dashboard Search API](https://grafana.com/docs/grafana/latest/developers/http_api/folder_dashboard_search/)  
- [Folder API](https://grafana.com/docs/grafana/latest/developers/http_api/folder/)  
- [Dashboard API](https://grafana.com/docs/grafana/latest/developers/http_api/dashboard/)  
- [Data Source API](https://grafana.com/docs/grafana/latest/developers/http_api/data_source/)  

## License

This project is licensed under the [MIT License](LICENSE). See the LICENSE file for details.
