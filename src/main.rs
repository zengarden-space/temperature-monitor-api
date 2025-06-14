use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tower_http::cors::CorsLayer;
use tracing::{info, warn};

const VICTORIA_METRICS_URL: &str = "http://vmsingle-vm-victoria-metrics-k8s-stack.victoria-metrics.svc:8429";

#[derive(Debug, Serialize, Deserialize)]
struct PrometheusResponse {
    status: String,
    data: PrometheusData,
}

#[derive(Debug, Serialize, Deserialize)]
struct PrometheusData {
    #[serde(rename = "resultType")]
    result_type: String,
    result: Vec<PrometheusResult>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PrometheusResult {
    metric: HashMap<String, String>,
    value: (f64, String),
}

#[derive(Debug, Serialize, Deserialize)]
struct TemperatureMeasurement {
    node: String,
    minutely_temperature: f64,
    hourly_temperature: f64,
    daily_temperature: f64,
}

#[derive(Debug, Serialize)]
struct TemperatureResponse {
    measurements: Vec<TemperatureMeasurement>,
}

#[derive(Debug, Deserialize)]
struct QueryParams {
    // Optional parameter to use localhost for development
    #[serde(default)]
    dev: bool,
}

async fn get_temperatures(Query(params): Query<QueryParams>) -> Result<Json<TemperatureResponse>, StatusCode> {
    let client = Client::new();
    let base_url = if params.dev {
        "http://localhost:8429"
    } else {
        VICTORIA_METRICS_URL
    };

    // Get current timestamp
    let now = Utc::now();
    let _one_minute_ago = now - Duration::minutes(1);
    let _one_hour_ago = now - Duration::hours(1);
    let _one_day_ago = now - Duration::days(1);

    let mut blade_temperatures: HashMap<String, TemperatureMeasurement> = HashMap::new();

    // First, get the pod IP to node name mapping
    let ip_to_node_map = match get_pod_to_node_mapping(&client, base_url).await {
        Ok(map) => map,
        Err(e) => {
            warn!("Failed to get pod to node mapping: {}, using IP-based naming", e);
            HashMap::new()
        }
    };

    // Query for minutely maximum (last 1 minute)
    let minutely_query = format!(
        "max_over_time(node_hwmon_temp_celsius[1m])"
    );
    
    // Query for hourly maximum (last 1 hour)
    let hourly_query = format!(
        "max_over_time(node_hwmon_temp_celsius[1h])"
    );
    
    // Query for daily maximum (last 1 day)
    let daily_query = format!(
        "max_over_time(node_hwmon_temp_celsius[1d])"
    );

    // Fetch all three time ranges
    let (minutely_result, hourly_result, daily_result) = tokio::try_join!(
        fetch_prometheus_data(&client, base_url, &minutely_query),
        fetch_prometheus_data(&client, base_url, &hourly_query),
        fetch_prometheus_data(&client, base_url, &daily_query)
    ).map_err(|e| {
        warn!("Failed to fetch temperature data: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Process results and group by blade server (using pod IP to node mapping)
    process_temperature_data(minutely_result, hourly_result, daily_result, &mut blade_temperatures, &ip_to_node_map);

    // Convert to vector and sort by node name
    let mut measurements: Vec<TemperatureMeasurement> = blade_temperatures.into_values().collect();
    measurements.sort_by(|a, b| a.node.cmp(&b.node));

    Ok(Json(TemperatureResponse {
        measurements,
    }))
}



async fn get_pod_to_node_mapping(
    client: &Client,
    base_url: &str,
) -> Result<HashMap<String, String>, anyhow::Error> {
    let url = format!("{}/api/v1/query", base_url);
    let query = "kube_pod_info";
    
    let response = client
        .get(&url)
        .query(&[("query", query)])
        .send()
        .await?
        .json::<PrometheusResponse>()
        .await?;

    if response.status != "success" {
        return Err(anyhow::anyhow!("Prometheus query failed"));
    }

    let mut ip_to_node_map = HashMap::new();
    
    info!("Found {} kube_pod_info entries", response.data.result.len());
    
    for result in response.data.result {
        // Only process node-exporter pods
        if let Some(pod_name) = result.metric.get("pod") {
            if pod_name.contains("node-exporter") {
                if let (Some(pod_ip), Some(node)) = (
                    result.metric.get("pod_ip"),
                    result.metric.get("node")
                ) {
                    let instance = format!("{}:9100", pod_ip);
                    info!("Mapping pod IP {} (instance: {}) to node: {}", pod_ip, instance, node);
                    ip_to_node_map.insert(instance, node.clone());
                }
            }
        }
    }
    
    info!("Final ip_to_node_map has {} entries: {:?}", ip_to_node_map.len(), ip_to_node_map);

    Ok(ip_to_node_map)
}

async fn fetch_prometheus_data(
    client: &Client,
    base_url: &str,
    query: &str,
) -> Result<Vec<PrometheusResult>, anyhow::Error> {
    let url = format!("{}/api/v1/query", base_url);
    let response = client
        .get(&url)
        .query(&[("query", query)])
        .send()
        .await?
        .json::<PrometheusResponse>()
        .await?;

    if response.status != "success" {
        return Err(anyhow::anyhow!("Prometheus query failed"));
    }

    Ok(response.data.result)
}

fn process_temperature_data(
    minutely: Vec<PrometheusResult>,
    hourly: Vec<PrometheusResult>,
    daily: Vec<PrometheusResult>,
    blade_temperatures: &mut HashMap<String, TemperatureMeasurement>,
    ip_to_node_map: &HashMap<String, String>,
) {
    // Create lookup maps for faster access
    let minutely_map: HashMap<String, f64> = minutely
        .into_iter()
        .filter_map(|result| {
            let instance = result.metric.get("instance")?.clone();
            let temp: f64 = result.value.1.parse().ok()?;
            Some((instance, temp))
        })
        .collect();

    let hourly_map: HashMap<String, f64> = hourly
        .into_iter()
        .filter_map(|result| {
            let instance = result.metric.get("instance")?.clone();
            let temp: f64 = result.value.1.parse().ok()?;
            Some((instance, temp))
        })
        .collect();

    let daily_map: HashMap<String, f64> = daily
        .into_iter()
        .filter_map(|result| {
            let instance = result.metric.get("instance")?.clone();
            let temp: f64 = result.value.1.parse().ok()?;
            Some((instance, temp))
        })
        .collect();

    // Aggregate temperatures by instance (group multiple sensors per blade)
    let mut instance_groups: HashMap<String, Vec<(f64, f64, f64)>> = HashMap::new();

    for instance in minutely_map.keys() {
        let minutely_temp = minutely_map.get(instance).copied().unwrap_or(0.0);
        let hourly_temp = hourly_map.get(instance).copied().unwrap_or(0.0);
        let daily_temp = daily_map.get(instance).copied().unwrap_or(0.0);

        instance_groups
            .entry(instance.clone())
            .or_default()
            .push((minutely_temp, hourly_temp, daily_temp));
    }

    // Create blade names using the IP to node mapping and maximum temperatures
    for (instance, temps) in instance_groups {
        let blade_name = instance_to_blade_name(&instance, ip_to_node_map);
        
        if !temps.is_empty() {
            let (max_min, max_hour, max_day) = temps.iter().fold((f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |acc, &(m, h, d)| {
                (acc.0.max(m), acc.1.max(h), acc.2.max(d))
            });

            blade_temperatures.insert(
                blade_name.clone(),
                TemperatureMeasurement {
                    node: blade_name,
                    minutely_temperature: (max_min * 10.0).round() / 10.0, // Round to 1 decimal
                    hourly_temperature: max_hour.round(),                   // Round to integer
                    daily_temperature: (max_day * 10.0).round() / 10.0,    // Round to 1 decimal
                },
            );
        }
    }
}

fn instance_to_blade_name(instance: &str, ip_to_node_map: &HashMap<String, String>) -> String {
    info!("Looking up instance: {} in mapping", instance);
    // Try to get the blade name from the IP to node mapping using the full instance (IP:port)
    if let Some(node_name) = ip_to_node_map.get(instance) {
        info!("Found mapping: {} -> {}", instance, node_name);
        node_name.clone()
    } else {
        warn!("No mapping found for instance: {}, available keys: {:?}", instance, ip_to_node_map.keys().collect::<Vec<_>>());
        "unknown_blade".to_string()        
    }
}

async fn health_check() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Build application router
    let app = Router::new()
        .route("/", get(health_check))
        .route("/health", get(health_check))
        .route("/api/temperatures", get(get_temperatures))
        .layer(CorsLayer::permissive());

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind to port 3000");

    info!("Temperature Monitor API Server starting on http://0.0.0.0:3000");
    info!("Endpoints:");
    info!("  GET /                 - Health check");
    info!("  GET /health           - Health check");
    info!("  GET /api/temperatures - Get blade server temperatures");
    info!("  GET /api/temperatures?dev=true - Use localhost:8429 for development");

    axum::serve(listener, app)
        .await
        .expect("Failed to start server");
}
