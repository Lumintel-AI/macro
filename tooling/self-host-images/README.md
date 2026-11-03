# Self-host images

Dispatch Self-host images on self-host in this public fork. Select all eight
groups or retry a failed group with its cache. images.tsv is the 37-image
contract shared with the private deployment Compose files. Configuration and
infrastructure stay in that repository.

Build-only is the default on push and dispatch. Opt in with `push_images: true`;
`image_registry` defaults to `ecr`. Only the publish job gets `id-token: write`.
Set repository variables, not secrets, from the deployment stack outputs:
`ECR_PUSH_ROLE_ARN` from `AWS_ROLE_ARN`, `AWS_REGION` from `AWS_REGION`,
`ECR_REGISTRY` from `ECR_REGISTRY`, and `ECR_REPOSITORY_PREFIX` from
`ECR_REPOSITORY_PREFIX`. The push role trusts this repository's `self-host`
branch and audience `sts.amazonaws.com`. No static AWS keys are used.

Images use `<account>.dkr.ecr.us-east-2.amazonaws.com/<prefix>/<image>:<full-source-sha>`.
Each image's runtime layers and configuration pass the secret audit immediately
before its push. Audit reports and resource samples are workflow artifacts.
Deployment secrets never enter the builder. The VM pulls private ECR images
through its IAM role, with no stored registry token.

GHCR remains a fallback via `image_registry: ghcr`, using the workflow token and
`ghcr.io/lumintel-ai/macro/<image>:<full-source-sha>`. Select `imageRegistry: ghcr`
in deployment config and preview it separately. Private GHCR pulls require a
read token; public packages require a separate reviewed visibility change.

Rust builds reuse registry, git and target cache mounts; native dependency stages
also use layer caching. The generic group shares a builder across its binaries.
Each group has a measured 300 minute build limit and 30 minutes for cache and
artifacts. The old private runner killed Vite after transforming 14,983 modules,
exit 137 with cannot allocate memory. Public runners provide 16 GB and four CPUs.
WASM compilation stays serial. No workflow here deploys or provisions resources.
