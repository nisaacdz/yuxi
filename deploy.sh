#!/bin/bash
set -e

RESOURCE_GROUP="rg-yuxi-prod"
LOCATION="eastus"
CONTAINER_REGISTRY_NAME="acryuxiregistry$(openssl rand -hex 4)" # Must be globally unique
CONTAINER_APP_ENV_NAME="cae-yuxi-prod"
CONTAINER_APP_NAME="app-yuxi-prod"
LOCAL_DOCKER_IMAGE="yuxi-image:v1"
ENV_FILE=".env"

CONTAINER_TARGET_PORT=8000 

# ==============================================================================
# HELPER FUNCTION FOR CONVERTING ENV VAR NAMES
# ==============================================================================
# Azure Container App secret names must be lowercase alphanumeric characters or '-'.
# This function converts 'MY_VAR_NAME' to 'my-var-name'.
convert_to_secret_name() {
  echo "$1" | tr '[:upper:]' '[:lower:]' | tr '_' '-'
}

# ==============================================================================
# CREATE AZURE RESOURCES
# ==============================================================================
echo ">>> Creating resource group: $RESOURCE_GROUP..."
az group create --name "$RESOURCE_GROUP" --location "$LOCATION"

echo ">>> Creating Azure Container Registry (ACR): $CONTAINER_REGISTRY_NAME..."
az acr create \
  --name "$CONTAINER_REGISTRY_NAME" \
  --resource-group "$RESOURCE_GROUP" \
  --sku Basic \
  --admin-enabled true # Using admin for simplicity; managed identity is better for production

# ==============================================================================
# PUSH DOCKER IMAGE TO AZURE CONTAINER REGISTRY
# ==============================================================================
echo ">>> Logging in to ACR..."
az acr login --name "$CONTAINER_REGISTRY_NAME"

ACR_LOGIN_SERVER=$(az acr show --name "$CONTAINER_REGISTRY_NAME" --query loginServer -o tsv)
ACR_IMAGE_TAG="$ACR_LOGIN_SERVER/$LOCAL_DOCKER_IMAGE"

echo ">>> Tagging image for ACR: $ACR_IMAGE_TAG..."
docker tag "$LOCAL_DOCKER_IMAGE" "$ACR_IMAGE_TAG"

echo ">>> Pushing image to ACR..."
docker push "$ACR_IMAGE_TAG"


# ==============================================================================
# PREPARE SECRETS AND ENVIRONMENT VARIABLES FOR DEPLOYMENT
# ==============================================================================
echo ">>> Preparing secrets from $ENV_FILE for deployment..."
SECRETS_ARGS=""
ENV_VARS_ARGS=""

# Read the .env file line by line
while IFS='=' read -r key value || [[ -n "$key" ]]; do
  # Skip empty lines or comments
  if [[ -z "$key" || "$key" == \#* ]]; then
    continue
  fi

  # Remove potential quotes from the value
  value="${value%\"}"
  value="${value#\"}"

  # Convert the env var name to a valid Azure secret name (e.g., DATABASE_URL -> database-url)
  secret_name=$(convert_to_secret_name "$key")

  # Build the --secrets argument string
  SECRETS_ARGS+=" $secret_name=\"$value\""

  # Build the --env-vars argument string, referencing the secret
  # Format: ENV_VAR_NAME=secretref:azure-secret-name
  ENV_VARS_ARGS+=" $key=secretref:$secret_name"

done < "$ENV_FILE"


# ==============================================================================
# CREATE THE CONTAINER APP ENVIRONMENT AND THE CONTAINER APP
# ==============================================================================
echo ">>> Creating Container App Environment: $CONTAINER_APP_ENV_NAME..."
az containerapp env create \
  --name "$CONTAINER_APP_ENV_NAME" \
  --resource-group "$RESOURCE_GROUP" \
  --location "$LOCATION"

echo ">>> Creating Container App: $CONTAINER_APP_NAME..."
echo ">>> This step performs the entire deployment with secrets configured."

# The --secrets and --env-vars arguments are passed here.
# We use `eval` to correctly pass the space-separated arguments we built.
eval az containerapp create \
  --name "$CONTAINER_APP_NAME" \
  --resource-group "$RESOURCE_GROUP" \
  --environment "$CONTAINER_APP_ENV_NAME" \
  --image "$ACR_IMAGE_TAG" \
  --registry-server "$ACR_LOGIN_SERVER" \
  --registry-username "$CONTAINER_REGISTRY_NAME" \
  --registry-password "$(az acr credential show --name $CONTAINER_REGISTRY_NAME --query passwords[0].value -o tsv)" \
  --target-port "$CONTAINER_TARGET_PORT" \
  --ingress external \
  --min-replicas 1 \
  --max-replicas 1 \
  --secrets $SECRETS_ARGS \
  --env-vars $ENV_VARS_ARGS

# ==============================================================================
# VERIFY DEPLOYMENT
# ==============================================================================
APP_URL=$(az containerapp show --name $CONTAINER_APP_NAME --resource-group $RESOURCE_GROUP --query "properties.configuration.ingress.fqdn" -o tsv)

echo -e "\n\n🚀 Deployment complete!"
echo "-------------------------------------"
echo "Your application is available at:"
echo "https://""$APP_URL"
echo "-------------------------------------"
echo "To view live logs, run:"
echo "az containerapp logs show --name $CONTAINER_APP_NAME --resource-group $RESOURCE_GROUP --follow"
echo "-------------------------------------"