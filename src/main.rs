use std::env;
use std::fs;
use std::error::Error;

#[derive(Debug, serde::Deserialize)]
struct ServiceConfig {
    host: String,
    api_key: String,
}

#[derive(Debug, serde::Deserialize)]
struct Grafana {
    src: ServiceConfig,
    dst: ServiceConfig,
}

#[derive(Debug, serde::Deserialize)]
pub struct LogConfig {
    pub filepath: String
}

#[derive(Debug, serde::Deserialize)]
struct Config {
    grafana: Grafana,
    log_config: LogConfig,
}

impl Grafana {
    async fn export(&self) -> Result<(), Box<dyn Error>> {
        let current_dir = get_current_dir()?;

        let folders_dir = format!("{}/folders", current_dir);
        let dashboards_dir = format!("{}/dashboards", current_dir);
        let datasource_dir = format!("{}/datasources", current_dir);

        create_dir(&folders_dir)?;
        create_dir(&dashboards_dir)?;
        create_dir(&datasource_dir)?;

        let client = self.create_request_client(&self.src.api_key)?;

        self.export_folders_to_file(&client, &folders_dir).await?;
        self.export_dashboards_to_file(&client, &dashboards_dir).await?;
        self.export_datasources_to_file(&client, &datasource_dir).await?;

        Ok(())
    }

    async fn import(&self) -> Result<(), Box<dyn Error>> {
        let client = self.create_request_client(&self.dst.api_key)?;

        self.import_folders_from_files(&client).await?;
        self.import_dashboards_from_files(&client).await?;
        self.import_datasources_from_files(&client).await?;

        Ok(())
    }

    fn create_request_client(&self, api_key: &str) -> Result<reqwest::Client, Box<dyn Error>> {
        let auth_value = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", api_key)).map_err(|e| {
            log::error!("Failed to create AUTHORIZATION value: {}", e);
            e
        })?;

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::AUTHORIZATION, auth_value);

        let retry_policy = reqwest::retry::never().max_retries_per_request(5);

        let client = reqwest::Client::builder()
            .retry(retry_policy)
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| {
                log::error!("Failed to create HTTP client: {}", e);
                e
            })?;

        Ok(client)
    }

    async fn export_folders_to_file(
        &self,
        client: &reqwest::Client,
        folders_dir: &str
    ) -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = format!("{}/api/folders", self.src.host);
        let response = client.get(&endpoint).send().await?;

        let folder_uids: Vec<String> = response.json::<serde_json::Value>().await?
            .as_array()
            .map(|array| {
                array.iter().filter_map(|d| d["uid"].as_str()).map(|s| s.to_string()).collect()
            })
            .unwrap_or_default();


        for uid in folder_uids {
            let folders_dir = folders_dir.to_string();
            let endpoint = format!("{}/api/folders/{}", self.src.host, uid);

            match client.get(&endpoint).send().await {
                Err(e) => log::error!("Error fetching folder '{}' from '{}': {}", uid, endpoint, e),
                Ok(response) => {
                    if let Ok(mut json) = response.json::<serde_json::Value>().await {
                        let folder_json = json.as_object_mut().unwrap();

                        folder_json.remove("id");
                        folder_json.insert("overwrite".to_string(), serde_json::Value::Bool(true));

                        let file_path = format!("{}/{}.json", folders_dir, uid);

                        match fs::File::create(&file_path) {
                            Err(e) => log::error!("Error saving folder '{}': {}", uid, e),
                            Ok(f) => {
                                if let Err(e) = serde_json::to_writer(f, &folder_json) {
                                    log::error!("Error saving folder '{}': {}", uid, e);
                                } else {
                                    log::info!("Successfully saved folder: {}", file_path);
                                }
                            },
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn fetch_dashboard_uids(&self, client: &reqwest::Client) -> Result<Vec<String>, Box<dyn Error>> {
        let endpoint = format!("{}/api/search?type=dash-db", self.src.host);
        let response = client.get(&endpoint).send().await?;

        let dashboard_uids: Vec<String> = response.json::<serde_json::Value>().await?
            .as_array()
            .map(|array| array.iter().filter_map(|d| d["uid"].as_str()).map(|s| s.to_string()).collect())
            .unwrap_or_default();

        Ok(dashboard_uids)
    }

    async fn export_dashboards_to_file(
        &self,
        client: &reqwest::Client,
        dashboards_dir: &str
    ) -> Result<(), Box<dyn Error>> {
        let dashboard_uids = self.fetch_dashboard_uids(&client).await?;

        for uid in dashboard_uids {
            let dashboards_dir = dashboards_dir.to_string();
            let endpoint = format!("{}/api/dashboards/uid/{}", self.src.host, uid);

            match client.get(&endpoint).send().await {
                Err(e) => log::error!("Error fetching dashboard '{}' from '{}': {}", uid, endpoint, e),
                Ok(response) => {
                    if let Ok(mut json) = response.json::<serde_json::Value>().await {
                        let dashboard_json = json.as_object_mut().unwrap();
                        let folder_uid = dashboard_json["meta"]["folderUid"].as_str().unwrap().to_string();
                        let dashboard = &mut dashboard_json["dashboard"].as_object_mut().unwrap();

                        dashboard.remove("id");
                        dashboard_json.remove("meta");

                        dashboard_json.insert("folderUid".to_string(), serde_json::Value::String(folder_uid));
                        dashboard_json.insert("overwrite".to_string(), serde_json::Value::Bool(true));

                        let file_path = format!("{}/{}.json", dashboards_dir, uid);

                        match fs::File::create(&file_path) {
                            Err(e) => log::error!("Error saving dashboard '{}': {}", uid, e),
                            Ok(f) => {
                                if let Err(e) = serde_json::to_writer(f, &dashboard_json) {
                                    log::error!("Error saving dashboard '{}': {}", uid, e);
                                } else {
                                    log::info!("Successfully saved dashboard: {}", file_path);
                                }
                            },
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn export_datasources_to_file(
        &self,
        client: &reqwest::Client,
        datasources_dir: &str
    ) -> Result<(), Box<dyn std::error::Error>> {
        let endpoint = format!("{}/api/datasources", self.src.host);
        let response = client.get(&endpoint).send().await?;
        let datasources: Vec<serde_json::Value> = response.json::<serde_json::Value>().await?
            .as_array()
            .map(|array| {
                array.iter().filter_map(|item| {
                    let mut item_clone = item.clone();
                    item_clone.as_object_mut().map(|obj| {
                        obj.remove("id");
                        obj.remove("orgId");
                    });
                    Some(item_clone)
                }).collect()
            }).unwrap_or_default();

        for ds in datasources {
            if let Some(uid) = ds["uid"].as_str() {
                let file_path = format!("{}/{}.json", datasources_dir, uid);

                match fs::File::create(&file_path) {
                    Err(e) => log::error!("Failed to save datasource '{}': {}", uid, e),
                    Ok(f) => {
                        if let Err(e) = serde_json::to_writer(f, &ds) {
                            log::error!("Failed to save datasource '{}': {}", uid, e);
                        } else {
                            log::info!("Successfully saved datasource: {}", file_path);
                        }
                    },
                }
            }
        }

        Ok(())
    }

    async fn import_folders_from_files(&self, client: &reqwest::Client) -> Result<(), Box<dyn Error>> {
        let current_dir = get_current_dir()?;
        let folders_dir = format!("{}/folders", current_dir);

        for path in list_dir(&folders_dir) {
            let file_content = fs::read_to_string(&path).map_err(|e| {
                log::error!("Failed to read folder file '{}': {}", path.display(), e);
                e
            })?;

            let json_data: serde_json::Value = serde_json::from_str(&file_content)?;

            if let Some(folder_uid) = json_data.get("uid").and_then(|value| value.as_str()) {
                let mut endpoint = format!("{}/api/folders/{}", self.dst.host, folder_uid);
                let folder_exists = client.get(&endpoint)
                    .send()
                    .await
                    .map_or(false, |res| res.status() == reqwest::StatusCode::OK);

                let method = if folder_exists {
                    reqwest::Method::PUT
                } else {
                    endpoint = format!("{}/api/folders", self.dst.host);
                    reqwest::Method::POST
                };

                match client.request(method, &endpoint).json(&json_data).send().await {
                    Err(e) => log::error!("Error importing folder '{}' to '{}': {}", folder_uid, endpoint, e),
                    Ok(res) => {
                        log::info!("Importing folder '{}' to '{}' with status {}", folder_uid, endpoint, res.status());
                    }
                }
            }
        }

        Ok(())
    }

    async fn import_dashboards_from_files(&self, client: &reqwest::Client) -> Result<(), Box<dyn Error>> {
        let current_dir = get_current_dir()?;
        let dashboards_dir = format!("{}/dashboards", current_dir);

        for path in list_dir(&dashboards_dir) {
            let file_content = fs::read_to_string(&path).map_err(|e| {
                log::error!("Failed to read dashboard file '{}': {}", path.display(), e);
                e
            })?;

            let json_data: serde_json::Value = serde_json::from_str(&file_content)?;
            if let Some(dashboard_uid) = json_data["dashboard"].get("uid").and_then(|value| value.as_str()) {
                let endpoint = format!("{}/api/dashboards/db", self.dst.host);
                match client.post(&endpoint).json(&json_data).send().await {
                    Err(e) => log::error!("Error importing dashboard '{}' to '{}': {}", dashboard_uid, endpoint, e),
                    Ok(res) => log::info!("Importing dashboard '{}' to '{}' with status {}", dashboard_uid, endpoint, res.status()),
                }
            }
        }

        Ok(())
    }

    async fn import_datasources_from_files(&self, client: &reqwest::Client) -> Result<(), Box<dyn Error>> {
        let current_dir = get_current_dir()?;
        let datasources_dir = format!("{}/datasources", current_dir);
        let datasource_paths = list_dir(&datasources_dir);

        let endpoint = format!("{}/api/datasources", self.dst.host);
        let response = client.get(&endpoint).send().await?;
        let dst_datasource_uids: Vec<String> = response.json::<serde_json::Value>().await?
            .as_array()
            .map(|array| array.iter().filter_map(|item|
                item.get("uid").and_then(|uid| uid.as_str()).map(|s| s.to_string())
            ).collect())
            .unwrap_or_default();

        for path in datasource_paths {
            let file_content = fs::read_to_string(&path).map_err(|e| {
                log::error!("Failed to read datasource file '{}': {}", path.display(), e);
                e
            })?;

            let json_data: serde_json::Value = serde_json::from_str(&file_content)?;
            if let Some(datasource_uid) = json_data.get("uid").and_then(|value| value.as_str()) {
                let (method, endpoint) = if dst_datasource_uids.contains(&datasource_uid.to_string()) {
                    (reqwest::Method::PUT, format!("{}/api/datasources/uid/{}", self.dst.host, datasource_uid))
                } else {
                    (reqwest::Method::POST, format!("{}/api/datasources", self.dst.host))
                };

                match client.request(method, &endpoint).json(&json_data).send().await {
                    Err(e) => log::error!("Error importing datasource '{}' to '{}': {:?}", datasource_uid, endpoint, e),
                    Ok(res) => {
                        log::info!("Importing datasource '{}' to '{}' with status {}", datasource_uid, endpoint, res.status());
                    }
                }
            }
        }

        Ok(())
    }
}

fn create_dir(dir_path: &str) -> Result<(), Box<dyn Error>> {
    if !std::fs::exists(dir_path).unwrap_or(false) {
        fs::create_dir(dir_path).map_err(|e| {
            log::error!("Failed to create directory '{}': {}", dir_path, e);
            e
        })?;
    }

    Ok(())
}

fn list_dir(dir_path: &str) -> Vec<std::path::PathBuf> {
    match fs::read_dir(dir_path) {
        Ok(entries) => entries.filter_map(|entry| entry.ok().map(|e| e.path())).collect(),
        Err(e) => {
            log::error!("Failed to read directory '{}': {}", dir_path, e);
            Vec::new()
        },
    }
}

fn get_config(config_file: &str) -> Result<Config, Box<dyn Error>> {
    let file = std::fs::File::open(config_file).map_err(|e| {
        log::error!("Failed to open config gile '{}': {}", config_file, e);
        e
    })?;

    let config: Config = serde_yaml::from_reader(file)?;
    Ok(config)
}

fn get_current_dir() -> Result<String, Box<dyn Error>> {
    let current_dir = env::current_dir().map_err(|e|{
        log::error!("Failed to get current dir: {}", e);
        e
    })?;

    current_dir.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| {
            let message = "Failed to convert current dir to string".to_string();
            log::error!("{}", message);
            message.into()
        })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config_file = std::env::var("CONFIG_FILE")
        .expect("environment variable `CONFIG_FILE` not found");

    let config = get_config(&config_file)?;
    let (log_config, grafana) = (config.log_config, config.grafana);

    log4rs::init_file(log_config.filepath, Default::default())
        .expect("failed to init log4rs");

    grafana.export().await?;
    grafana.import().await?;

    Ok(())
}
