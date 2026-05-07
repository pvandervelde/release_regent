---
title: Server API
description: HTTP endpoints exposed by rr-server
---

# Server API

`rr-server` exposes two HTTP endpoints.

## `GET /`

Health check endpoint.

### Request

No parameters, headers, or body required.

```bash
curl http://your-server:8080/
```

### Response

**200 OK**

```json
{"status": "healthy"}
```

This endpoint is designed for container orchestration liveness and readiness probes. It always
returns `200 OK` as long as the server process is running and able to accept connections.

---

## `POST /webhook`

GitHub webhook receiver.

### Request

**Headers**:

| Header | Required | Description |
| :--- | :---: | :--- |
| `Content-Type` | ✅ | Must be `application/json` |
| `X-GitHub-Event` | ✅ | GitHub event type, e.g. `pull_request` |
| `X-Hub-Signature-256` | ✅ | HMAC-SHA256 signature of the body |
| `X-GitHub-Delivery` | | Unique delivery ID (used for logging) |

**Body**: Standard GitHub webhook JSON payload. Maximum size: 10 MiB.

### Responses

| Status | Body | Description |
| :--- | :--- | :--- |
| `200 OK` | `{"status": "processing"}` or `{"status": "ok"}` | Event accepted |
| `400 Bad Request` | `{"error": "<description>"}` | Malformed payload |
| `401 Unauthorized` | `{"error": "signature validation failed"}` | Invalid or missing signature |
| `403 Forbidden` | `{"error": "repository not allowed"}` | Repository blocked by `ALLOWED_REPOS` |
| `413 Payload Too Large` | — | Body exceeds 10 MiB |
| `500 Internal Server Error` | `{"error": "<description>"}` | Processing error |

### Asynchronous processing

Release Regent responds to the webhook request before processing is complete. The event is
placed on an internal queue and processed asynchronously. This ensures the server always
responds within GitHub's 10-second timeout, regardless of how long GitHub API calls take.

A `200 OK` response means the event was accepted and queued, not that the release workflow
completed.
