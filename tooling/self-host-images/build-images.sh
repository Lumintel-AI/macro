#!/usr/bin/env bash
set -euo pipefail

: "${BUILD_GROUP:?Set BUILD_GROUP}"
: "${IMAGE_TAG:?Set IMAGE_TAG}"
: "${SOURCE_SHA:?Set SOURCE_SHA}"
: "${PUSH_IMAGES:=false}"
: "${SOURCE_REPOSITORY_URL:=https://github.com/Lumintel-AI/macro}"
: "${ECR_REPOSITORY_PREFIX:=lumintel-macro}"
: "${ECR_REGISTRY:=local}"

mkdir -p measurements
cp tooling/self-host-images/images.tsv measurements/images.tsv
printf 'image\tseconds\tresult\n' > measurements/builds.tsv
failed=0
while IFS=$'\t' read -r image dockerfile; do
  if [[ "$BUILD_GROUP" == rust ]]; then
    [[ "$dockerfile" == docker/Dockerfile ]] || continue
  else
    [[ "$image" == "$BUILD_GROUP" ]] || continue
  fi
  context=.
  file="$dockerfile"
  web_args=()
  if [[ "$image" == web ]]; then
    context=.
    web_args=(
      --build-arg "VITE_PUBLIC_ORIGIN=${VITE_PUBLIC_ORIGIN:-https://delegate.lumintel.ai}"
      --build-arg "VITE_FUSIONAUTH_CLIENT_ID=${VITE_FUSIONAUTH_CLIENT_ID:-bb8570aa-1529-5eaf-aea7-084078c76620}"
      --build-arg "VITE_FUSIONAUTH_TENANT_ID=${VITE_FUSIONAUTH_TENANT_ID:-c46c1f71-3fb9-5b56-903c-1a40e7fe9a27}"
      --build-arg "VITE_SYNC_SERVICE_HOST=${SYNC_SERVICE_HOST:-delegate.lumintel.ai/sync}"
    )
  fi
  tag="$ECR_REGISTRY/$ECR_REPOSITORY_PREFIX/$image:$IMAGE_TAG"
  output=(--load)
  start=$SECONDS
  result=0
  # Rust cache mounts persist across the generic images on this builder.
  # cache-dance restores and saves those mounts between workflow runs.
  docker buildx build "$context" --file "$file" --platform linux/amd64 \
    --build-arg "SERVICE_NAME=$image" \
    "${web_args[@]}" \
    --label "org.opencontainers.image.revision=$SOURCE_SHA" \
    --label "org.opencontainers.image.source=$SOURCE_REPOSITORY_URL" \
    --label 'io.lumintel.application-source=https://github.com/Lumintel-AI/macro' \
    --tag "$tag" --progress plain "${output[@]}" \
    --cache-from "type=gha,scope=delegate-$image" \
    --cache-to "type=gha,scope=delegate-$image,mode=max,ignore-error=true" \
    --metadata-file "measurements/$image.metadata.json" \
    2>&1 | tee "measurements/$image.log" || result=$?
  printf '%s\t%s\t%s\n' "$image" "$((SECONDS-start))" "$result" | tee -a measurements/builds.tsv
  if [[ "$result" != 0 ]]; then
    failed=1
    # Stop this group on its first failure. Other matrix jobs keep running.
    break
  fi
  python3 tooling/self-host-images/audit-image.py "$tag" "measurements/$image.audit.json"
  if [[ "$PUSH_IMAGES" == true ]]; then docker push "$tag"; fi
  if [[ "$PUSH_IMAGES" != true ]]; then
    docker image inspect "$tag" --format '{{json .}}' > "measurements/$image.image.json"
    if [[ "$image" == opensearch ]]; then
      docker run --rm --entrypoint /usr/share/opensearch/bin/opensearch-plugin "$tag" list | grep -Fx analysis-icu
    elif [[ "$image" == web ]]; then
      docker run -d --name delegate-web-check -p 127.0.0.1:8099:80 "$tag"
      trap 'docker rm -f delegate-web-check >/dev/null 2>&1 || true' EXIT
      for _attempt in {1..10}; do
        if curl -fsS http://127.0.0.1:8099/app > measurements/web-app.html; then break; fi
        sleep 1
      done
      grep -q '<html' measurements/web-app.html
      curl -fsS http://127.0.0.1:8099/app/deep/link > measurements/web-deep-link.html
      cmp measurements/web-app.html measurements/web-deep-link.html
      docker rm -f delegate-web-check
      trap - EXIT
    fi
  fi
  docker image rm "$tag"
done < measurements/images.tsv
exit "$failed"
