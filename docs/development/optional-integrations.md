# Optional integrations

The backend can run with external macrod sessions and no deployment model API keys. Each person signs in to Claude Code or Codex on their own machine and pairs macrod. Managed model calls, dictation and duplicate detection require their own configured providers.

Omit unused credentials entirely. An explicitly configured remote secret must still resolve successfully, and malformed configured credentials fail validation. No dummy credentials are needed.

| Capability | Optional configuration | Behavior while absent |
| --- | --- | --- |
| Hosted model calls | `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `CEREBRAS_API_KEY`, `FIREWORK_API_KEY`, `GOOGLE_GENERATIVE_AI_API_KEY` | Selected unconfigured provider returns unavailable. External sessions remain usable. |
| Managed sandboxes | `DAYTONA_API_KEY`, `ANTHROPIC_API_KEY` | Provisioning, resume and resize return unavailable. |
| Dictation | `OPENAI_API_KEY` | HTTP 503. |
| Task duplicate detection | `OPENAI_API_KEY`, `COHERE_API_KEY` | Similarity search returns 503; new tasks skip detection; saved matches remain readable. |
| GitHub App | `GITHUB_SYNC_APP_URL`, `GITHUB_SYNC_APP_CLIENT_ID`, `GITHUB_SYNC_APP_CLIENT_SECRET`, `GITHUB_INSTALLATION_STATE_SECRET`, `GITHUB_WEBHOOK_SECRET_KEY`, `GITHUB_SYNC_APP_PEM_SECRET_KEY` | App routes and installation-token operations are unavailable. Stored pull request metadata remains readable. |
| GitHub pull request patches | `GITHUB_PULL_REQUEST_PATCH_BUCKET` | Optional in DSS and the harness. Patch operations are unavailable without a bucket. |
| GitHub account linking | `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET`, `GITHUB_IDP_ID` | OAuth operations are unavailable. |
| Stripe | `STRIPE_SECRET_KEY`, `STRIPE_PRICE_ID`, `STRIPE_WEBHOOK_SECRET_KEY` | Signups save no customer identity; payment operations are unavailable. Apply the `optional_stripe_customer` migration before these services. |
| LiveKit | `LIVEKIT_SERVER_URL`, `LIVEKIT_API_KEY`, `LIVEKIT_API_SECRET` | Call routes return 503; persisted records remain usable. |
| Cal conversion webhooks | `CAL_WEBHOOK_SECRET_KEY`, `CAL_EVENT_TYPE_CONTENT_NAMES_KEY`, `META_PIXEL_ID`, `META_ACCESS_TOKEN` | Webhook route returns 503. |
| Pipedream | `PIPEDREAM_CLIENT_ID`, `PIPEDREAM_CLIENT_SECRET`, `PIPEDREAM_PROJECT_ID` | Connected-app operations are unavailable; built-in and custom MCP remain independent. |
| Apollo | `APOLLO_API_KEY` | Email enrichment uses the existing unfurl implementation. |

GitHub App and LiveKit groups require all their credentials to enable the integration. Cal webhooks require both Cal secrets and the Meta configuration. The existing secret-name versus resolved-value conventions still apply.

## Document job callbacks

`JOB_UPDATE_TRANSPORT` accepts `lambda` or `connection_gateway`. It defaults to `lambda`, which requires `WEB_SOCKET_RESPONSE_LAMBDA` and preserves upstream asynchronous invocation.

`connection_gateway` requires the existing `INTERNAL_API_KEY`. Its destination comes from the typed `ConnectionGatewayUrl`, including `OVERRIDE_CONNECTION_GATEWAY_URL`. It sends the original camelCase job result in a `job_update` message to `/message/send/document/{document_id}`. Gateway delivery derives the document id from the typed destination S3 key. Lambda delivery still accepts arbitrary destination keys. Unzip already has the document id.

The gateway client carries fleet authentication, a 15-second timeout and tracing headers. A non-success HTTP status fails delivery. Workers acknowledge successful jobs only after callback delivery succeeds. Failed callback delivery propagates for redrive. A terminal unzip failure that was reported successfully retains upstream acknowledgement behavior. Status messages can be repeated after redelivery, so consumers must treat job id and status as repeatable updates.

The legacy document-processing WebSocket client is a separate transport. This callback does not recreate that retired WebSocket service or its request-submission protocol.
