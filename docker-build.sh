#!/bin/bash

set -euo pipefail
IFS=$'\n\t'

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
cd $SCRIPT_DIR

PROJ_NAME=$(basename $SCRIPT_DIR)

DOCKER_REGISTRY=${DOCKER_REGISTRY:=registry.gitlab.com}
DOCKER_REPOSITORY=${DOCKER_REPOSITORY:="nickbp/rpi/${PROJ_NAME}:"}
DOCKER_BUILD=${DOCKER_BUILD:="sudo img build --platform linux/amd64,linux/arm64"}

# Get 7-character commit SHA (note: doesn't detect dirty commits)
COMMIT_SHA=$(git rev-parse HEAD | cut -b 1-7)

time /bin/sh -c "$DOCKER_BUILD -t ${DOCKER_REGISTRY}/${DOCKER_REPOSITORY}${COMMIT_SHA} ."
