#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 Deploying Fuse to Kubernetes${NC}\n"

# Check prerequisites
echo -e "${YELLOW}Checking prerequisites...${NC}"
command -v kubectl >/dev/null 2>&1 || { echo -e "${RED}kubectl is required but not installed${NC}"; exit 1; }
command -v helm >/dev/null 2>&1 || { echo -e "${RED}helm is required but not installed${NC}"; exit 1; }

# Step 1: Install Cilium
echo -e "\n${GREEN}Step 1: Installing Cilium with Hubble${NC}"
helm repo add cilium https://helm.cilium.io/
helm repo update

helm upgrade --install cilium cilium/cilium --version 1.14.5 \
  --namespace kube-system \
  --set hubble.enabled=true \
  --set hubble.relay.enabled=true \
  --set hubble.ui.enabled=true \
  --set hubble.metrics.enabled="{dns,drop,tcp,flow,icmp,http}" \
  --set hubble.export.fileMaxSizeMb=10 \
  --set hubble.export.fileMaxBackups=5 \
  --set prometheus.enabled=true \
  --set operator.prometheus.enabled=true \
  --set encryption.enabled=true \
  --set encryption.type=wireguard \
  --set kubeProxyReplacement=strict \
  --set k8sServiceHost=kubernetes.default.svc \
  --set k8sServicePort=443

echo -e "${GREEN}✓ Cilium installed${NC}"

# Wait for Cilium to be ready
echo "Waiting for Cilium to be ready..."
kubectl wait --for=condition=ready pod -l k8s-app=cilium -n kube-system --timeout=300s

# Step 2: Install Cert-Manager
echo -e "\n${GREEN}Step 2: Installing Cert-Manager${NC}"
helm repo add jetstack https://charts.jetstack.io
helm repo update

helm upgrade --install cert-manager jetstack/cert-manager \
  --namespace cert-manager \
  --create-namespace \
  --version v1.13.3 \
  --set installCRDs=true

echo -e "${GREEN}✓ Cert-Manager installed${NC}"

# Step 3: Install Envoy Gateway
echo -e "\n${GREEN}Step 3: Installing Envoy Gateway${NC}"
helm repo add envoy-gateway https://gateway.envoyproxy.io
helm repo update

kubectl create namespace gateway-system --dry-run=client -o yaml | kubectl apply -f -

helm upgrade --install envoy-gateway envoy-gateway/gateway-helm \
  --namespace gateway-system \
  --version v0.6.0

echo -e "${GREEN}✓ Envoy Gateway installed${NC}"

# Step 4: Create observability namespace
echo -e "\n${GREEN}Step 4: Setting up Observability Stack${NC}"
kubectl create namespace observability --dry-run=client -o yaml | kubectl apply -f -

# Install OpenTelemetry Operator
kubectl apply -f https://github.com/open-telemetry/opentelemetry-operator/releases/latest/download/opentelemetry-operator.yaml

# Apply observability components
kubectl apply -f observability/opentelemetry-collector.yaml
kubectl apply -f observability/prometheus.yaml
kubectl apply -f observability/loki.yaml
kubectl apply -f observability/grafana.yaml

echo -e "${GREEN}✓ Observability stack deployed${NC}"

# Step 5: Create Fuse namespace and deploy
echo -e "\n${GREEN}Step 5: Deploying Fuse Application${NC}"
kubectl apply -f namespace.yaml
kubectl apply -f serviceaccount.yaml
kubectl apply -f configmap.yaml
kubectl apply -f pvc.yaml
kubectl apply -f deployment.yaml
kubectl apply -f service.yaml

echo -e "${GREEN}✓ Fuse application deployed${NC}"

# Step 6: Apply Cilium policies
echo -e "\n${GREEN}Step 6: Applying Cilium Network Policies${NC}"
kubectl apply -f cilium/network-policy.yaml
kubectl apply -f cilium/hubble-config.yaml

echo -e "${GREEN}✓ Network policies applied${NC}"

# Step 7: Configure Envoy Gateway
echo -e "\n${GREEN}Step 7: Configuring Envoy Gateway${NC}"
kubectl apply -f envoy/gateway.yaml

echo -e "${GREEN}✓ Gateway configured${NC}"

# Step 8: Create Grafana admin secret
echo -e "\n${GREEN}Step 8: Creating Grafana admin credentials${NC}"
kubectl create secret generic grafana-admin \
  --from-literal=username=admin \
  --from-literal=password=$(openssl rand -base64 32) \
  --namespace observability \
  --dry-run=client -o yaml | kubectl apply -f -

GRAFANA_PASSWORD=$(kubectl get secret grafana-admin -n observability -o jsonpath='{.data.password}' | base64 -d)

echo -e "${GREEN}✓ Grafana admin password: ${GRAFANA_PASSWORD}${NC}"

# Wait for deployments
echo -e "\n${YELLOW}Waiting for deployments to be ready...${NC}"
kubectl wait --for=condition=available deployment/fuse-api -n fuse-system --timeout=300s
kubectl wait --for=condition=available deployment/grafana -n observability --timeout=300s

# Display status
echo -e "\n${GREEN}═══════════════════════════════════════════════════${NC}"
echo -e "${GREEN}✓ Deployment Complete!${NC}"
echo -e "${GREEN}═══════════════════════════════════════════════════${NC}\n"

echo -e "${YELLOW}Access URLs:${NC}"
echo "  Fuse API: kubectl port-forward -n fuse-system svc/fuse-api 8080:8080"
echo "  Grafana: kubectl port-forward -n observability svc/grafana 3000:3000"
echo "  Prometheus: kubectl port-forward -n observability svc/prometheus 9090:9090"
echo "  Hubble UI: kubectl port-forward -n kube-system svc/hubble-ui 12000:80"

echo -e "\n${YELLOW}Grafana Credentials:${NC}"
echo "  Username: admin"
echo "  Password: ${GRAFANA_PASSWORD}"

echo -e "\n${YELLOW}Useful Commands:${NC}"
echo "  View Fuse logs: kubectl logs -n fuse-system -l app=fuse-api -f"
echo "  View Hubble flows: hubble observe -n fuse-system"
echo "  Check Cilium status: cilium status"
echo "  View metrics: kubectl top pods -n fuse-system"

echo -e "\n${GREEN}Done!${NC}"
