# Self-host images

Dispatch Self-host images on self-host in this public fork. Select all eight
groups or retry a failed group with its cache. images.tsv is the 37-image
contract shared with the private deployment Compose files. Configuration and
infrastructure stay in that repository.

Images use ghcr.io/lumintel-ai/macro/<image>:<full-source-sha>. Every runtime
layer and image configuration is inspected before pushing. Audit reports and
host resource samples are workflow artifacts. Review audits before separately
changing package visibility. Deployment secrets never enter the builder.

Rust builds reuse registry, git and target cache mounts; native dependency stages
also use layer caching. The generic group shares a builder across its binaries.
Each group has a measured 300 minute build limit and 30 minutes for cache and
artifacts. The old private runner killed Vite after transforming 14,983 modules,
exit 137 with cannot allocate memory. Public runners provide 16 GB and four CPUs.
WASM compilation stays serial. No workflow here deploys or provisions resources.
