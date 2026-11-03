# Self-host images

Dispatch Self-host images on self-host in this public fork. Select all eight
groups or retry a failed group with its cache. images.tsv is the 37-image
contract shared with the private deployment Compose files. Configuration and
infrastructure stay in that repository.

Build-only is the default on push and dispatch. Opt in with `push_images: true`;
`image_registry` defaults to `ecr`. Publication runs only on push or dispatch
on `self-host` in `Lumintel-AI/macro`; fork pull requests cannot publish.
Set secrets `AWS_ECR_PUSH_ACCESS_KEY_ID` and `AWS_ECR_PUSH_SECRET_ACCESS_KEY`
with the deployment repository's post-apply `scripts/ci-push-key.sh` helper.
It sets `AWS_REGION` and `ECR_REGISTRY` repository variables. Set
`ECR_REPOSITORY_PREFIX` from the stack output if using a custom prefix.
The dedicated IAM user can push only to the deployment's 37 ECR repositories.
No job requests `id-token: write`. The deployment stack also supports `oidc`
for accounts that permit it; this workflow uses access keys. An OIDC publisher
must configure role assumption and grant its publish job `id-token: write`.

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
