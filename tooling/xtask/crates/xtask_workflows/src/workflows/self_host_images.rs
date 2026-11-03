//! Public-runner image builds, isolated from deployment workflows.

pub fn render() -> anyhow::Result<String> {
    let mut workflow: serde_yaml::Value = serde_yaml::from_str(
        r###"
name: Self-host images
run-name: Images ${{ github.sha }} / ${{ inputs.build_group || 'all' }}
permissions:
  contents: read
concurrency:
  group: self-host-images-${{ github.ref }}-${{ inputs.build_group }}
  cancel-in-progress: false
jobs:
  plan:
    runs-on: ubuntu-24.04
    timeout-minutes: 5
    outputs:
      matrix: ${{ steps.plan.outputs.matrix }}
      source_sha: ${{ github.sha }}
      image_tag: ${{ github.sha }}
    steps:
    - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262
      with:
        persist-credentials: false
    - id: plan
      env:
        SELECTED_GROUP: ${{ inputs.build_group || 'all' }}
      run: 'set -euo pipefail

        [[ "$GITHUB_REF" == refs/heads/self-host && "$GITHUB_REPOSITORY" == Lumintel-AI/macro ]] || exit 1

        python3 tooling/self-host-images/plan.py >> "$GITHUB_OUTPUT"

        '
  build:
    needs: plan
    strategy:
      fail-fast: false
      matrix: ${{ fromJSON(needs.plan.outputs.matrix) }}
    runs-on: ubuntu-24.04
    timeout-minutes: 330
    permissions:
      contents: read
      packages: write
      id-token: write
    env:
      BUILD_GROUP: ${{ matrix.group }}
      IMAGE_TAG: ${{ needs.plan.outputs.image_tag }}
      SOURCE_SHA: ${{ needs.plan.outputs.source_sha }}
      PUSH_IMAGES: ${{ inputs.push_images || false }}
      REGISTRY_KIND: ${{ inputs.image_registry || 'ecr' }}
      ECR_REGISTRY: ${{ inputs.image_registry == 'ghcr' && 'ghcr.io' || vars.ECR_REGISTRY || 'local' }}
      ECR_REPOSITORY_PREFIX: ${{ inputs.image_registry == 'ghcr' && 'lumintel-ai/macro' || vars.ECR_REPOSITORY_PREFIX || 'lumintel-macro' }}
      VITE_PUBLIC_ORIGIN: ${{ vars.VITE_PUBLIC_ORIGIN || 'https://delegate.lumintel.ai' }}
      VITE_FUSIONAUTH_CLIENT_ID: ${{ vars.VITE_FUSIONAUTH_CLIENT_ID || 'bb8570aa-1529-5eaf-aea7-084078c76620' }}
      VITE_FUSIONAUTH_TENANT_ID: ${{ vars.VITE_FUSIONAUTH_TENANT_ID || 'c46c1f71-3fb9-5b56-903c-1a40e7fe9a27' }}
      SYNC_SERVICE_HOST: ${{ vars.SYNC_SERVICE_HOST || 'delegate.lumintel.ai/sync' }}
      SOURCE_REPOSITORY_URL: https://github.com/Lumintel-AI/macro
      CARGO_BUILD_JOBS: '3'
    steps:
    - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262
      with:
        persist-credentials: false
    - name: Free unused runner SDKs and start host measurements
      run: 'set -euo pipefail

        sudo rm -rf /usr/share/dotnet /opt/ghc /usr/local/lib/android /opt/hostedtoolcache/CodeQL

        mkdir -p measurements

        { uname -a; nproc; free -b; df -B1 .; docker version; } > measurements/runner.txt

        python3 tooling/self-host-images/measure.py measurements > measurements/monitor.log 2>&1 &

        echo "$!" > measurements/monitor.pid

        printf ''%s\n'' "$SOURCE_SHA" > measurements/source-sha.txt

        date -u +%FT%TZ > measurements/start.txt

        '
    - uses: docker/setup-buildx-action@8d2750c68a42422c14e847fe6c8ac0403b4cbd6f
    - name: Expose GitHub cache runtime to buildx CLI
      uses: crazy-max/ghaction-github-runtime@3cb05d89e1f492524af3d41a1c98c83bc3025124
    - name: Cache Rust registry, git sources and release target
      if: matrix.group == 'rust' || matrix.group == 'web' || matrix.group == 'document_text_extractor_vm'
      id: rust-cache
      uses: actions/cache@0057852bfaa89a56745cba8c7296529d2fc39830
      with:
        path: .ci-cache
        key: rust-mounts-v1-${{ matrix.group }}-${{ hashFiles('Cargo.lock', 'rust-toolchain.toml', 'tooling/self-host-images/web.Dockerfile')
          }}-${{ needs.plan.outputs.source_sha }}
        restore-keys: 'rust-mounts-v1-${{ matrix.group }}-${{ hashFiles(''Cargo.lock'', ''rust-toolchain.toml'', ''tooling/self-host-images/web.Dockerfile'')
          }}-

          ${{ matrix.group == ''rust'' && ''rust-mounts-v1-document_text_extractor_vm-'' || matrix.group == ''document_text_extractor_vm''
          && ''rust-mounts-v1-rust-'' || '''' }}

          '
    - name: Restore BuildKit Rust cache mounts
      if: matrix.group == 'rust' || matrix.group == 'web' || matrix.group == 'document_text_extractor_vm'
      uses: reproducible-containers/buildkit-cache-dance@7a38cfab60c63ee1022b7c1a14a031c82f4114c7
      with:
        cache-map: "{\n  \".ci-cache/registry\": \"/usr/local/cargo/registry\",\n  \".ci-cache/git\": \"/usr/local/cargo/git/db\"\
          ,\n  \".ci-cache/target\": {\"target\": \"/app/target\", \"id\": \"${{ matrix.group == 'web' && 'web-wasm-target'\
          \ || 'rust-target-release' }}\", \"sharing\": \"locked\"}\n}\n"
        skip-extraction: ${{ steps.rust-cache.outputs.cache-hit }}
    - name: Validate ECR publication variables
      if: inputs.push_images && inputs.image_registry != 'ghcr'
      env:
        ROLE_ARN: ${{ vars.ECR_PUSH_ROLE_ARN }}
        REGION: ${{ vars.AWS_REGION }}
      run: |
        set -euo pipefail
        : "${ROLE_ARN:?Set ECR_PUSH_ROLE_ARN from Pulumi AWS_ROLE_ARN}"
        : "${REGION:?Set AWS_REGION}"
        [[ "$ECR_REGISTRY" != local ]]
    - name: Assume ECR push role through GitHub OIDC
      if: inputs.push_images && inputs.image_registry != 'ghcr'
      uses: aws-actions/configure-aws-credentials@e7f100cf4c008499ea8adda475de1042d6975c7b
      with:
        role-to-assume: ${{ vars.ECR_PUSH_ROLE_ARN }}
        aws-region: ${{ vars.AWS_REGION }}
        audience: sts.amazonaws.com
    - name: Log in to private ECR
      if: inputs.push_images && inputs.image_registry != 'ghcr'
      run: |
        set -euo pipefail
        aws ecr get-login-password --region "$AWS_REGION" | docker login "$ECR_REGISTRY" --username AWS --password-stdin
    - name: Log in to GHCR with the workflow token
      if: inputs.push_images && inputs.image_registry == 'ghcr'
      env:
        GHCR_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      run: 'set -euo pipefail

        printf ''%s'' "$GHCR_TOKEN" | docker login ghcr.io --username "$GITHUB_ACTOR" --password-stdin

        '
    - name: Build and audit images, with a measured 300 minute limit
      timeout-minutes: 302
      run: 'set -euo pipefail

        result=0

        timeout --signal=TERM --kill-after=30s 300m tooling/self-host-images/build-images.sh || result=$?

        printf ''%s\n'' "$result" > measurements/group-exit-code.txt

        exit "$result"

        '
    - name: Write measurements even after a build failure or timeout
      if: always()
      run: 'set -euo pipefail

        touch measurements/stop

        python3 tooling/self-host-images/measure-summary.py measurements | tee -a "$GITHUB_STEP_SUMMARY"

        cat measurements/builds.tsv >> "$GITHUB_STEP_SUMMARY" || true

        docker buildx du > measurements/buildkit-disk.txt || true

        docker system df > measurements/docker-disk.txt || true

        sudo dmesg | tee measurements/kernel.txt >/dev/null || true

        date -u +%FT%TZ > measurements/end.txt

        '
    - name: Upload build logs and resource samples
      if: always()
      uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02
      with:
        name: image-measurements-${{ matrix.group }}-${{ github.run_attempt }}
        path: measurements/
        retention-days: 14
'on':
  push:
    branches: [self-host]
    paths: ['.github/workflows/self-host-images.yml', 'tooling/self-host-images/**']
  workflow_dispatch:
    inputs:
      build_group:
        description: All images or one failed group
        type: string
        default: all
      image_registry:
        description: Publication registry, ECR by default; GHCR fallback
        type: choice
        options: [ecr, ghcr]
        default: ecr
      push_images:
        description: Publish audited images to the selected registry
        type: boolean
        default: false
"###,
    )?;
    // Share the measured build steps, but grant OIDC only to opt-in publication.
    let mut publish = workflow["jobs"]["build"].clone();
    publish["if"] = serde_yaml::Value::String("${{ inputs.push_images == true }}".into());
    workflow["jobs"]["publish"] = publish;
    workflow["jobs"]["build"]["if"] =
        serde_yaml::Value::String("${{ inputs.push_images != true }}".into());
    workflow["jobs"]["build"]["permissions"] = serde_yaml::from_str("contents: read")?;
    Ok(serde_yaml::to_string(&workflow)?)
}
