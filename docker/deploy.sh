#!/bin/bash
set -uo pipefail

# Get login token and execute login
sudo pip install awscli
$(aws ecr get-login --no-include-email --region $AWS_REGION)

echo "Tagging latest image with solver...";
docker build --tag $REGISTRY_URI:$1 -f docker/Dockerfile .
echo "Pushing image";
docker push $REGISTRY_URI:$1
