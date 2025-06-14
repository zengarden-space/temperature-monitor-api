<!-- Use this file to provide workspace-specific custom instructions to Copilot. For more details, visit https://code.visualstudio.com/docs/copilot/copilot-customization#_use-a-githubcopilotinstructionsmd-file -->

# Temperature Monitor API Server

This is a Rust API server that monitors blade server temperatures by querying Victoria Metrics Prometheus API.

## Project Context
- Uses Victoria Metrics service at: http://vmsingle-vm-victoria-metrics-k8s-stack.victoria-metrics.svc:8429
- Queries node exporter metrics for temperature data
- Aggregates data into minutely, hourly, and daily averages
- Returns JSON format: {"blade001": {"minutely":73.3, "hourly":74, "daily":83.2}, "blade002": ...}

## Technical Stack
- Rust with tokio for async runtime
- Axum web framework for REST API
- Reqwest for HTTP client
- Serde for JSON serialization
- Prometheus query language (PromQL) for metrics aggregation

## Key Components
- Temperature data fetching from Victoria Metrics
- Data aggregation and calculation
- REST API endpoints
- Error handling and logging
