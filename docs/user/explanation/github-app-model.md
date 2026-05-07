---
title: GitHub App authentication
description: Why Release Regent uses a GitHub App and how JWT-based authentication works
---

# GitHub App authentication

Release Regent authenticates to GitHub using a **GitHub App** rather than a personal access
token (PAT). This article explains the difference, describes the token exchange flow, and
clarifies what permissions are requested and why.

## GitHub App vs. personal access token

| Aspect | Personal access token | GitHub App |
| :--- | :--- | :--- |
| Identity | Tied to a human user account | Acts as a bot identity |
| Scope | Set at token creation | Defined by the App's permission manifest |
| Rotation | Manual | Automatic (installation tokens expire after 1 hour) |
| Auditability | Actions attributed to the user | Actions attributed to the App name |
| Rate limits | Shared with the user | Separate rate limit pool per installation |
| Installation | N/A | Must be explicitly installed on each repository |

For an automation tool that commits to repositories and creates PRs on behalf of teams, a
GitHub App is the appropriate choice. It avoids tying production access to a single developer's
account and provides a clear audit trail.

## The JWT-based token exchange flow

GitHub App authentication works in two stages:

```
┌──────────────────┐         ┌──────────────────┐
│  Release Regent  │         │      GitHub       │
└────────┬─────────┘         └────────┬──────────┘
         │                            │
         │ 1. Sign JWT with App ID    │
         │    and RSA private key     │
         │──────────────────────────►│
         │                            │
         │ 2. Exchange JWT for        │
         │    installation token      │
         │◄──────────────────────────│
         │                            │
         │ 3. Use installation token  │
         │    for API calls           │
         │──────────────────────────►│
         │                            │
```

1. **JWT generation**: The server creates a short-lived JSON Web Token (JWT) signed with the
   private key (from `GITHUB_PRIVATE_KEY`). The JWT contains the App ID (from
   `GITHUB_APP_ID`) and expires after 10 minutes.

2. **Installation token exchange**: The server presents the JWT to GitHub's authentication
   endpoint and requests an **installation access token** scoped to the specific repository
   that sent the webhook (identified by `installation.id` in the payload).

3. **API calls**: All GitHub API calls use the installation token, which is valid for 1 hour.
   The server caches tokens and refreshes them before expiry.

This flow means the long-lived private key is never sent over the network — only the short-lived
JWT and the even shorter-lived installation token leave the server.

## Required permissions

Release Regent requests the minimum permissions needed to perform its work:

| Permission | Level | Reason |
| :--- | :--- | :--- |
| Contents | Read & write | Create Git tags and fetch commit history |
| Issues | Read | Link related issues in changelogs |
| Metadata | Read | Read repository name, default branch, etc. |
| Pull requests | Read & write | Create release PRs, post status comments |

It requests no account or organisation permissions. It cannot access repositories it is not
explicitly installed on, and it cannot act on behalf of users.

## The installation ID and multi-repository support

Every GitHub App webhook payload includes an `installation.id` field — the ID of the GitHub
App installation that was active when the event was generated. Release Regent reads this ID
from the incoming payload and uses it to request an installation token scoped to exactly that
installation's repositories.

This means a single running server can serve webhooks from multiple GitHub App installations
(e.g., multiple organisations) without any extra configuration. The correct credentials are
derived automatically from each incoming event.

## Key rotation

If the private key is compromised or expires, generate a new key in GitHub App settings and
update `GITHUB_PRIVATE_KEY` in the server deployment. The old key and new key can coexist
briefly during rollover. Delete the old key from GitHub App settings once the server is
confirmed healthy with the new key.

---

## Related reading

- [Set up the GitHub App](../how-to/setup/github-app-setup.md)
- [Environment variables](../reference/environment-variables.md)
