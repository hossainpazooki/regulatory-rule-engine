# Local Kubernetes Development

This document covers deploying the legal-compliance-applied-ai application to a local Kubernetes cluster using minikube.

## Prerequisites

- Docker Desktop for Windows
- minikube (`winget install minikube`)
- kubectl (`winget install kubectl`)
- Temporal CLI (`winget install temporalio.temporal-cli`) - for workflow testing

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     minikube cluster                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   frontend   │──►    api      │  │    worker    │      │
│  │   (nginx)    │  │  (FastAPI)   │  │  (Temporal)  │      │
│  │   :8080      │  │    :8000     │  │   Running    │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                 │               │
│    NodePort:30000    NodePort:30080         │               │
└─────────┼─────────────────┼─────────────────┼───────────────┘
          │                 │                 │
          │    minikube tunnel               │
          │   (exposes NodePorts)            │
          ▼                 ▼                 ▼
    localhost:30000   localhost:30080   host.minikube.internal:7233
                                              │
                            ┌─────────────────┴─────────────────┐
                            │  Temporal Dev Server (host)       │
                            │  temporal server start-dev        │
                            │  localhost:7233 (gRPC)            │
                            │  localhost:8233 (Web UI)          │
                            └───────────────────────────────────┘

Internal traffic flow:
  frontend ──/api/──► api:8000 (nginx proxies to backend service)
  worker ──────────► host.minikube.internal:7233 (Temporal on host)
```

## Quick Start

```powershell
# 1. Navigate to project directory
cd legal-compliance-applied-ai

# 2. Start minikube with Docker driver
minikube start --driver=docker

# 3. Point shell to minikube's Docker daemon
& minikube -p minikube docker-env --shell powershell | Invoke-Expression

# 4. Build images inside minikube
docker build -t legal-compliance-api:local .
docker build -t legal-compliance-worker:local -f Dockerfile.worker .
docker build -t legal-compliance-frontend:local .

# 5. Deploy with kustomize
kubectl apply -k kube/overlays/local

# 6. Start Temporal dev server (in separate terminal)
temporal server start-dev

# 7. Expose services to localhost (in separate terminal - keep running)
minikube tunnel

# 8. Access the application
# Frontend: http://localhost:30000
# API Docs: http://localhost:30080/docs
# API Health: http://localhost:30080/health
# Temporal UI: http://localhost:8233
```

**Note for Windows users:** Step 7 (`minikube tunnel`) is required because NodePort services are not directly accessible on localhost with Docker driver. Keep the tunnel terminal running while developing.

**Alternative to minikube tunnel - Port forwarding:**

```powershell
# If tunnel doesn't work, use port forwarding instead:
Start-Process powershell -ArgumentList '-Command', 'kubectl port-forward svc/frontend 30000:80 -n legal-compliance'
Start-Process powershell -ArgumentList '-Command', 'kubectl port-forward svc/api 30080:8000 -n legal-compliance'
```

## Kustomize Overlay Structure

```
kube/
├── base/                    # Production-ready base configuration
│   ├── api/
│   │   ├── deployment.yaml
│   │   ├── service.yaml
│   │   ├── serviceaccount.yaml
│   │   ├── hpa.yaml
│   │   └── external-secret.yaml
│   ├── worker/
│   │   ├── deployment.yaml
│   │   ├── serviceaccount.yaml
│   │   └── hpa.yaml
│   ├── frontend/
│   │   ├── deployment.yaml
│   │   ├── service.yaml
│   │   └── hpa.yaml
│   ├── namespace.yaml
│   └── kustomization.yaml
└── overlays/
    └── local/               # Local development overrides
        ├── kustomization.yaml
        ├── configmap.yaml
        └── secrets.yaml
```

## Local Overlay Configuration

The `kube/overlays/local/kustomization.yaml` applies these modifications:

| Patch                              | Purpose                        |
| ---------------------------------- | ------------------------------ |
| Single replica for all deployments | Reduce resource usage          |
| Reduced CPU/memory requests        | 50m CPU, 128Mi memory          |
| NodePort services                  | External access (30000, 30080) |
| Delete ExternalSecret              | Use local secrets.yaml instead |
| Delete HPAs                        | Not needed locally             |
| SQLite volume mount                | EmptyDir for /app/data         |
| Disable readOnlyRootFilesystem     | Allow SQLite writes            |

## Issues Encountered & Solutions

### 1. Frontend CrashLoopBackOff - nginx permission denied

**Error:**

```
mkdir() "/var/cache/nginx/client_temp" failed (13: Permission denied)
```

**Cause:** Base nginx image requires root to create cache directories. Kubernetes runs containers as non-root (UID 1000).

**Solution:** Changed `Dockerfile` (now at repo root) to use `nginxinc/nginx-unprivileged:alpine`:

```dockerfile
FROM nginxinc/nginx-unprivileged:alpine
COPY --from=build /app/dist /usr/share/nginx/html
USER root
COPY nginx.conf /etc/nginx/conf.d/default.conf
RUN chown -R nginx:nginx /usr/share/nginx/html
USER nginx
EXPOSE 8080
```

Also updated `nginx.conf` to listen on port 8080 (unprivileged port).

### 2. Docker build using cached layers

**Symptom:** After changing Dockerfile, `docker build` showed "CACHED" for all layers and image ID didn't change.

**Solution:** Force rebuild without cache:

```powershell
docker build --no-cache -t legal-compliance-frontend:local .
```

### 3. ServiceAccount not found

**Error:**

```
serviceaccount "api-sa" not found
```

**Cause:** Base deployment referenced ServiceAccounts that didn't exist.

**Solution:** Created ServiceAccount resources:

- `kube/base/api/serviceaccount.yaml`
- `kube/base/worker/serviceaccount.yaml`

Added them to `kube/base/kustomization.yaml`.

### 4. API CrashLoopBackOff - SQLite database error

**Error:**

```
sqlite3.OperationalError: unable to open database file
```

**Cause:** Two issues:

1. `readOnlyRootFilesystem: true` prevented writing to container filesystem
2. No volume mounted for database storage

**Solution:** Added patches in local overlay:

```yaml
# Mount volume for SQLite
- patch: |-
    apiVersion: apps/v1
    kind: Deployment
    metadata:
      name: api
    spec:
      template:
        spec:
          containers:
            - name: api
              volumeMounts:
                - name: data
                  mountPath: /app/data
          volumes:
            - name: data
              emptyDir: {}
  target:
    kind: Deployment
    name: api

# Disable read-only filesystem
- patch: |-
    - op: replace
      path: /spec/template/spec/containers/0/securityContext/readOnlyRootFilesystem
      value: false
  target:
    kind: Deployment
    name: api
```

Updated `DATABASE_URL` in configmap to use absolute path:

```yaml
DATABASE_URL: "sqlite:////app/data/ke_workbench.db"
```

### 5. NodePort not accessible on localhost

**Symptom:** `http://localhost:30000` unreachable even though NodePort configured.

**Cause:** minikube with Docker driver on Windows doesn't expose NodePorts directly to localhost.

**Solution:** Use kubectl port-forwarding:

```powershell
kubectl port-forward svc/frontend 30000:80 -n legal-compliance
kubectl port-forward svc/api 30080:8000 -n legal-compliance
```

### 6. Worker CrashLoopBackOff - Temporal Connection

**Error:**

```
Failed to connect to Temporal at temporal-frontend.temporal.svc.cluster.local:7233
```

**Cause:** Worker tries to connect to a Kubernetes service that doesn't exist. For local dev, we run Temporal on the host machine.

**Solution:**

1. Update ConfigMap to use host-accessible address:
   
   ```yaml
   TEMPORAL_HOST: "host.minikube.internal:7233"
   ```

2. Start Temporal dev server on host:
   
   ```powershell
   temporal server start-dev
   ```

### 7. Worker Sandbox Validation Error

**Error:**

```
RestrictedWorkflowAccessError: Cannot access datetime.datetime.utcnow.__wrapped__
RuntimeError: Failed validating workflow ComplianceCheckWorkflow
```

**Cause:** Temporal's workflow sandbox restricts non-deterministic operations like `datetime.utcnow`. Imported modules (pydantic schemas) use this.

**Solution:** Configure sandbox to pass through application modules in `worker.py`:

```python
from temporalio.worker.workflow_sandbox import (
    SandboxedWorkflowRunner,
    SandboxRestrictions,
)

sandbox_runner = SandboxedWorkflowRunner(
    restrictions=SandboxRestrictions.default.with_passthrough_modules(
        "backend",
        "pydantic",
    ),
)

worker = Worker(
    client,
    task_queue=task_queue,
    workflows=WORKFLOWS,
    activities=ACTIVITIES,
    workflow_runner=sandbox_runner,
)
```

### 8. Frontend Cannot Reach Backend API (ERR_CONNECTION_REFUSED)

**Error:**

```
Failed to load rules
GET http://localhost:30000/api/rules net::ERR_CONNECTION_REFUSED
```

**Cause:** The frontend nginx container proxies `/api/` requests to the backend, but the backend service name resolution or proxy configuration fails.

**Investigation steps:**

```powershell
# 1. Verify API is healthy from inside cluster
kubectl exec -it deployment/frontend -n legal-compliance -- wget -qO- http://api:8000/health
# Should return: {"status":"healthy"}

# 2. Verify DNS resolution
kubectl exec -it deployment/frontend -n legal-compliance -- nslookup api
# Should show: api.legal-compliance.svc.cluster.local

# 3. Check nginx proxy configuration
kubectl exec -it deployment/frontend -n legal-compliance -- cat /etc/nginx/conf.d/default.conf
```

**Solution:** Use simple nginx proxy configuration. The key insight is that Kubernetes internal DNS works reliably for service discovery, so complex resolver directives are unnecessary and can cause issues.

**Working nginx.conf:**

```nginx
server {
    listen 8080;
    server_name localhost;
    root /usr/share/nginx/html;
    index index.html;

    # Security headers
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "DENY" always;
    add_header X-XSS-Protection "1; mode=block" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self';" always;

    # Handle client-side routing
    location / {
        try_files $uri $uri/ /index.html;
    }

    # Proxy API requests to backend
    location /api/ {
        proxy_pass http://api:8000/;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_connect_timeout 30s;
        proxy_send_timeout 30s;
        proxy_read_timeout 30s;
    }

    # Cache static assets
    location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2)$ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    # Gzip compression
    gzip on;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml application/xml+rss text/javascript;
}
```

**Common mistakes to avoid:**

- Don't add `resolver` directives for Kubernetes DNS - they can break service discovery
- Don't use variables for backend hostnames (e.g., `set $backend http://api:8000`) - this requires resolver configuration
- Keep `proxy_pass http://api:8000/;` simple with the trailing slash to strip `/api/` prefix

### 9. Windows minikube: localhost Refused to Connect

**Error:** Browser shows "localhost refused to connect" when accessing `http://localhost:30000` even though pods are running.

**Cause:** On Windows with minikube using Docker driver, NodePort services are not automatically exposed to `localhost`. The services exist within the minikube VM/container, not on the host network.

**Solution:** Run `minikube tunnel` to expose NodePort services:

```powershell
# In a separate terminal (keep it running)
minikube tunnel
```

This creates a network tunnel that routes traffic from localhost to minikube's services.

**Alternative - Get minikube service URL:**

```powershell
# Get the actual URL to access the service
minikube service frontend -n legal-compliance --url
# Example output: http://127.0.0.1:58793
```

**Alternative - Port forwarding (already documented above):**

```powershell
kubectl port-forward svc/frontend 30000:80 -n legal-compliance
```

**Note:** The `minikube tunnel` approach is preferred because:

- NodePorts work as expected (e.g., `http://localhost:30000`)
- Multiple services accessible simultaneously
- No need for individual port-forward commands

## Observability

### Kubernetes Dashboard

```powershell
minikube dashboard
```

Select namespace `legal-compliance` in the dropdown.

### Temporal Web UI

When running `temporal server start-dev`, access the workflow UI at:

- http://localhost:8233

Features:

- View workflow executions
- Inspect workflow history and state
- Query workflow progress
- Trigger signals

### Logs

```powershell
# Follow API logs
kubectl logs -f deployment/api -n legal-compliance

# Follow frontend logs
kubectl logs -f deployment/frontend -n legal-compliance

# Follow worker logs
kubectl logs -f deployment/worker -n legal-compliance
```

### Pod Status

```powershell
kubectl get pods -n legal-compliance -w
```

## Current Status

| Component | Status  | Notes                            |
| --------- | ------- | -------------------------------- |
| Frontend  | Running | nginx-unprivileged on port 8080  |
| API       | Running | FastAPI with SQLite              |
| Worker    | Running | Connected to Temporal dev server |
| Temporal  | Running | Dev server on host machine       |

## Environment Variables (Local)

From `kube/overlays/local/configmap.yaml`:

| Variable             | Value                                                               |
| -------------------- | ------------------------------------------------------------------- |
| ENVIRONMENT          | local                                                               |
| ENABLE_VECTOR_SEARCH | false                                                               |
| TEMPORAL_HOST        | host.minikube.internal:7233                                         |
| TEMPORAL_NAMESPACE   | default                                                             |
| TEMPORAL_TASK_QUEUE  | compliance-workflows                                                |
| CORS_ORIGINS         | http://localhost:30000,http://127.0.0.1:30000,http://localhost:3000 |
| LOG_LEVEL            | DEBUG                                                               |
| DATABASE_URL         | sqlite:////app/data/ke_workbench.db                                 |

## Cleanup

```powershell
# Delete all resources
kubectl delete -k kube/overlays/local

# Stop minikube
minikube stop

# Delete minikube cluster entirely
minikube delete
```

## Troubleshooting

### Check pod events

```powershell
kubectl describe pod <pod-name> -n legal-compliance
```

### Check service endpoints

```powershell
kubectl get endpoints -n legal-compliance
```

### Rebuild and redeploy

```powershell
# Ensure using minikube's Docker
& minikube -p minikube docker-env --shell powershell | Invoke-Expression

# Rebuild without cache
docker build --no-cache -t legal-compliance-api:local .

# Restart deployment to pick up new image
kubectl rollout restart deployment/api -n legal-compliance
```

### Reset everything

```powershell
minikube delete
minikube start --driver=docker
# Then follow Quick Start steps
```

### Rebuild after nginx.conf changes

After modifying `nginx.conf` (now at repo root), rebuild and redeploy:

```powershell
# Ensure using minikube's Docker
& minikube -p minikube docker-env --shell powershell | Invoke-Expression

# Rebuild frontend image (--no-cache ensures nginx.conf changes are picked up)
docker build --no-cache -t legal-compliance-frontend:local .

# Restart frontend deployment
kubectl rollout restart deployment/frontend -n legal-compliance

# Watch pods come up
kubectl get pods -n legal-compliance -w
```

### Test API connectivity from inside cluster

To debug frontend-to-backend connectivity issues:

```powershell
# Check if API is reachable from frontend pod
kubectl exec -it deployment/frontend -n legal-compliance -- wget -qO- http://api:8000/health

# Check DNS resolution
kubectl exec -it deployment/frontend -n legal-compliance -- nslookup api

# Check current nginx config
kubectl exec -it deployment/frontend -n legal-compliance -- cat /etc/nginx/conf.d/default.conf
```
