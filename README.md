# Temperature Monitor API Server

A Rust-based API server that monitors blade server temperatures by querying Victoria Metrics Prometheus API.

## Overview

This service fetches temperature data from node exporter metrics stored in Victoria Metrics and provides aggregated temperature readings for blade servers in minutely, hourly, and daily averages.

## API Endpoints

- `GET /` - Health check endpoint
- `GET /health` - Health check endpoint  
- `GET /temperatures` - Get blade server temperatures
- `GET /temperatures?dev=true` - Use localhost:8429 for development (with port-forward)

## Response Format

```json
{
  "measurements": [
    {
      "node": "blade001",
      "minutely_temperature": 73.3,
      "hourly_temperature": 74,
      "daily_temperature": 83.2
    },
    {
      "node": "blade002", 
      "minutely_temperature": 71.5,
      "hourly_temperature": 72,
      "daily_temperature": 81.8
    }
  ]
}
```

## Configuration

The service connects to Victoria Metrics at:
- Production: `http://vmsingle-vm-victoria-metrics-k8s-stack.victoria-metrics.svc:8429`
- Development: `http://localhost:8429` (when using `?dev=true` parameter)

## Deployment

### CI/CD Pipeline

The project includes a complete CI/CD pipeline using GitHub Actions (adapted for Gitea) that:

1. **Builds and pushes Docker images** to `gitea.zengarden.space/zengarden-space/temperature-monitor-api`
2. **Lints the Helm chart** for validation
3. **Generates Kubernetes manifests** using Helm templates
4. **Deploys to the cluster** via GitOps (pushes to manifests repository)

### Helm Chart

The service is deployed using a Helm chart located in `./helm/temperature-monitor-api/` with:

- **Deployment** with 2 replicas (adjustable via HPA)
- **Service** exposing port 80 → 3000
- **Ingress** with SSL termination (Let's Encrypt)
- **HPA** for auto-scaling (2-5 replicas)
- **Health checks** on `/health` endpoint
- **Security contexts** with non-root user

### Environment-specific Values

- **Development**: `./helm/values-dev.yaml` - Exposes at `temperature-monitor-api.zengarden.space`
- **Branch deployments**: `./helm/values-branch.yaml` - Exposes at `temperature-monitor-api.branch-{ID}.zengarden.space`

### Manual Deployment

To deploy manually:

```bash
# Build and push image
docker build -t gitea.zengarden.space/zengarden-space/temperature-monitor-api:latest .
docker push gitea.zengarden.space/zengarden-space/temperature-monitor-api:latest

# Deploy with Helm
helm upgrade --install temperature-monitor-api ./helm/temperature-monitor-api \
  --namespace temperature-monitor-api \
  --create-namespace \
  --values ./helm/values-dev.yaml
```

## Development

### Prerequisites

- Rust (1.70+)
- kubectl access to Victoria Metrics service
- Port-forward for local development

### Running Locally

1. Port-forward the Victoria Metrics service:
```bash
kubectl port-forward -n victoria-metrics svc/vmsingle-vm-victoria-metrics-k8s-stack 8429:8429
```

2. Build and run the server:
```bash
cargo run
```

3. Test the API:
```bash
# Health check
curl http://localhost:8081/health

# Get temperatures (using port-forward)
curl "http://localhost:8081/temperatures?dev=true"
```

### Building for Production

```bash
cargo build --release
```

## Temperature Data Sources

The service queries the following Prometheus metrics:
- `node_hwmon_temp_celsius` - Hardware monitoring temperature sensors
- Uses `avg_over_time()` function for time-based aggregation:
  - Minutely: `avg_over_time(node_hwmon_temp_celsius[1m])`
  - Hourly: `avg_over_time(node_hwmon_temp_celsius[1h])`  
  - Daily: `avg_over_time(node_hwmon_temp_celsius[1d])`

## Troubleshooting

1. **Connection errors**: Ensure Victoria Metrics service is accessible
2. **No data**: Check that node exporters are running and collecting temperature metrics
3. **Port conflicts**: Change the server port in main.rs if needed
