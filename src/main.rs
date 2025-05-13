use std::env;
use std::fs;
use std::sync::OnceLock;

static CONFIG: OnceLock<EnvConfig> = OnceLock::new();

#[derive(Debug)]
struct EnvConfig {
    grafana_src_host: String,
    grafana_src_api_key: String,
    grafana_dst_host: String,
    grafana_dst_api_key: String,
}

fn create_dir(dir_path: &str) {
    if let Ok(exist) = fs::exists(dir_path) {
        if !exist {
            fs::create_dir(dir_path).unwrap();
        }
    }
}

fn list_dir(dir_path: &str) -> Vec<std::path::PathBuf> {
    fs::read_dir(dir_path).unwrap()
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, std::io::Error>>().unwrap()
}

async fn export_dashboard() {
    let config = &CONFIG.get().unwrap();

    let current_dir = {
        let current_dir = env::current_dir().unwrap();
        current_dir.to_str().unwrap().to_string()
    };

    let dashboards_dir = format!("{}/dashboards", current_dir);
    let folders_dir = format!("{}/folders", current_dir);

    create_dir(&dashboards_dir);
    create_dir(&folders_dir);

    let grafana_src_api_key = format!("Bearer {}", config.grafana_src_api_key);
    let auth_value = reqwest::header::HeaderValue::from_str(&grafana_src_api_key).unwrap();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, auth_value);

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let endpoint = format!("{}/api/search?type=dash-db", config.grafana_src_host);

    let dashboard_uids: Vec<String> = match client.get(&endpoint).send().await {
        Err(_) => Vec::new(),
        Ok(res) => {
            res.json::<serde_json::Value>().await
                .ok()
                .and_then(|json| json.as_array().cloned())
                .unwrap_or_default()
                .into_iter()
                .map(|d| d["uid"].as_str().unwrap().to_string())
                .collect()        
        },
    };

    let mut handles: Vec<tokio::task::JoinHandle<()>> = Vec::new();

    for uid in dashboard_uids {
        let client = client.clone();
        let dashboards_dir = dashboards_dir.clone();
        let endpoint = format!("{}/api/dashboards/uid/{}", config.grafana_src_host, uid);

        handles.push(tokio::spawn(async move {
            if let Ok(res) = client.get(&endpoint).send().await {
                if let Ok(mut json) = res.json::<serde_json::Value>().await {
                    let dashboard_json = json.as_object_mut().unwrap();
                    let folder_uid = dashboard_json["meta"]["folderUid"].as_str().unwrap().to_string();
                    let dashboard = &mut dashboard_json["dashboard"].as_object_mut().unwrap();

                    dashboard.remove("id");
                    dashboard_json.remove("meta");

                    dashboard_json.insert("folderUid".to_string(), serde_json::Value::String(folder_uid));
                    dashboard_json.insert("overwrite".to_string(), serde_json::Value::Bool(true));

                    let file = fs::File::create(format!("{}/{}.json", dashboards_dir, uid)).unwrap();
                    serde_json::to_writer_pretty(file, &dashboard_json).unwrap();

                    println!("Successfully saved dashboard: dashboards/{}.json", uid);
                }
            }
        }));
    }

    let endpoint = format!("{}/api/folders", config.grafana_src_host);

    let folder_uids: Vec<String> = match client.get(&endpoint).send().await {
        Err(_) => Vec::new(),
        Ok(res) => {
            res.json::<serde_json::Value>().await
                .ok()
                .and_then(|json| json.as_array().cloned())
                .unwrap_or_default()
                .into_iter()
                .map(|d| d["uid"].as_str().unwrap().to_string())
                .collect()        
        },
    };

    for uid in folder_uids {
        let client = client.clone();
        let folders_dir = folders_dir.clone();
        let endpoint = format!("{}/api/folders/{}", config.grafana_src_host, uid);

        handles.push(tokio::spawn(async move {
            if let Ok(res) = client.get(&endpoint).send().await {
                if let Ok(mut json) = res.json::<serde_json::Value>().await {
                    let folder_json = json.as_object_mut().unwrap();

                    folder_json.remove("id");
                    folder_json.insert("overwrite".to_string(), serde_json::Value::Bool(true));

                    let file = fs::File::create(format!("{}/{}.json", folders_dir, uid)).unwrap();
                    serde_json::to_writer_pretty(file, &folder_json).unwrap();
                    println!("Successfully saved folder: folders/{}.json", uid);
                }
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

async fn import_dashboard() {
    let config = &CONFIG.get().unwrap();

    let current_dir = {
        let current_dir = env::current_dir().unwrap();
        current_dir.to_str().unwrap().to_string()
    };

    let folders_dir = format!("{}/folders", current_dir);
    let dashboards_dir = format!("{}/dashboards", current_dir);

    let grafana_dst_api_key = format!("Bearer {}", config.grafana_dst_api_key);
    let auth_value = reqwest::header::HeaderValue::from_str(&grafana_dst_api_key).unwrap();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, auth_value);

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let mut handles: Vec<tokio::task::JoinHandle<()>> =  Vec::new();

    let folders_path = list_dir(&folders_dir);

    for path in folders_path {
        let file_content = fs::read_to_string(path).unwrap();
        let json_data: serde_json::Value = serde_json::from_str(&file_content).unwrap();

        let folder_uid = json_data.get("uid").unwrap().as_str().unwrap().to_string();
        let mut endpoint = format!("{}/api/folders/{}", config.grafana_dst_host, folder_uid);
        let client = client.clone();
        let grafana_dst_host = config.grafana_dst_host.clone();

        handles.push(tokio::spawn(async move {
            let folder_exist: bool = client.get(&endpoint)
                .send()
                .await
                .map(|res| res.status() == reqwest::StatusCode::OK)
                .unwrap_or(false);

            let method = if folder_exist {
                reqwest::Method::PUT
            } else {
                endpoint = format!("{}/api/folders", grafana_dst_host);
                reqwest::Method::POST
            };

            match client.request(method, &endpoint).json(&json_data).send().await {
                Err(e) => eprintln!("Error importing folder '{}' to '{}': {}", folder_uid, endpoint, e),
                Ok(res) => {
                    if res.status() == reqwest::StatusCode::OK {
                        println!("Successfully importing dashboard '{}' to '{}'", folder_uid, endpoint);
                    } else {
                        eprintln!("Failed to import dashboard '{}' to '{}' with status code {}", folder_uid, endpoint, res.status().as_u16());
                    }
                },
            }
        }));
    }

    for handle in handles.drain(..) {
        handle.await.unwrap();
    }

    let dashboards_path = list_dir(&dashboards_dir);

    for path in dashboards_path {
        let file_content = fs::read_to_string(path).unwrap();
        let json_data: serde_json::Value = serde_json::from_str(&file_content).unwrap();
        let dashboard_uid = json_data["dashboard"].get("uid").unwrap().as_str().unwrap().to_string();

        let endpoint = format!("{}/api/dashboards/db", config.grafana_dst_host);
        let client = client.clone();

        handles.push(tokio::spawn(async move {
            match client.post(&endpoint).json(&json_data).send().await {
                Err(e) => eprintln!("Error importing dashboard '{}' to '{}': {}", dashboard_uid, endpoint, e),
                Ok(res) => {
                    if res.status() == reqwest::StatusCode::OK {
                        println!("Successfully importing dashboard '{}' to '{}'", dashboard_uid, endpoint);
                    } else {
                        eprintln!("Failed to import dashboard '{}' to '{}' with status code {}", dashboard_uid, endpoint, res.status().as_u16());
                    }
                },
            }
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }
    
    import_data_sources().await;
}

async fn import_data_sources() {
    let config = &CONFIG.get().unwrap();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap();

    let mut headers = reqwest::header::HeaderMap::new();
    let api_key = format!("Bearer {}", config.grafana_src_api_key);
    headers.insert(reqwest::header::AUTHORIZATION, reqwest::header::HeaderValue::from_str(&api_key).unwrap());

    let endpoint = format!("{}/api/datasources", config.grafana_src_host);

    let data_sources: Vec<serde_json::Value> = match client.get(&endpoint).headers(headers).send().await {
        Err(_) => vec![],
        Ok(res) => {
            res.json::<serde_json::Value>().await
                .ok()
                .and_then(|json| json.as_array().cloned())
                .unwrap_or_default()
                .into_iter()
                .map(|mut item| {
                    if let Some(obj) = item.as_object_mut() {
                        obj.remove("id");
                        obj.remove("orgId");
                    }

                    item
                })
                .collect()
        }
    };

    let mut headers = reqwest::header::HeaderMap::new();
    let api_key = format!("Bearer {}", config.grafana_dst_api_key);
    headers.insert(reqwest::header::AUTHORIZATION, reqwest::header::HeaderValue::from_str(&api_key).unwrap());

    let mut endpoint = format!("{}/api/datasources", config.grafana_dst_host);

    let dst_data_source_uids: Vec<serde_json::Value> = match client.get(&endpoint)
        .headers(headers.clone())
        .send().
        await {
            Err(_) => vec![],
            Ok(res) => {
                res.json::<serde_json::Value>().await
                    .ok()
                    .and_then(|json| json.as_array().cloned())
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|item| {
                        item.as_object()
                            .and_then(|obj| obj.get("uid").cloned())
                    })
                    .collect()
            }
    };

    for ds in data_sources {
        let uid = match ds["uid"].as_str() {
            Some(uid) => uid,
            None => continue,
        };

        let method = if dst_data_source_uids.contains(&serde_json::json!(uid)) {
            endpoint = format!("{}/api/datasources/uid/{}", config.grafana_dst_host, uid); 
            reqwest::Method::PUT
        } else {
            reqwest::Method::POST
        };

        client.request(method, &endpoint)
            .headers(headers.clone())
            .json(&ds)
            .send()
            .await
            .unwrap();
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    CONFIG.set(EnvConfig {
        grafana_src_host: std::env::var("GRAFANA_SRC_HOST").expect("GRAFANA_SRC_HOST must be set"),
        grafana_src_api_key: std::env::var("GRAFANA_SRC_API_KEY").expect("GRAFANA_SRC_API_KEY must be set"),
        grafana_dst_host: std::env::var("GRAFANA_DST_HOST").expect("GRAFANA_DST_HOST must be set"),
        grafana_dst_api_key: std::env::var("GRAFANA_DST_API_KEY").expect("GRAFANA_DST_API_KEY must be set"),
    }).unwrap();

    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <export | import>", args[0]);
    } else {
        match args[1].as_str() {
            "export" => export_dashboard().await,
            "import" => import_dashboard().await,
            other => eprintln!("{}: '{}' it's not a valid argument\n\nHere's the available argument:\n- export\n- import", args[0], other),
        }
    }
}
