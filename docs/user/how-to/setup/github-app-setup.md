---
title: Set up the GitHub App
description: How to create and configure the GitHub App that Release Regent uses to authenticate
---

# Set up the GitHub App

Release Regent authenticates to GitHub using a GitHub App. This page explains how to create,
configure, and install a GitHub App for your repositories.

For background on why Release Regent uses a GitHub App rather than a personal access token, see
[GitHub App authentication](../../explanation/github-app-model.md).

## Step 1: Create the GitHub App

1. Go to **GitHub → Settings → Developer settings → GitHub Apps → New GitHub App**
   (or `https://github.com/settings/apps/new`)

2. Fill in the basic details:

   | Field | Value |
   | :--- | :--- |
   | **App name** | `Release Regent - <your name or org>` (must be globally unique) |
   | **Homepage URL** | `https://github.com/pvandervelde/release_regent` |
   | **Webhook URL** | Leave blank for now — you will fill this in after deployment |
   | **Webhook secret** | A random string. Generate one: `openssl rand -hex 32` |

3. Under **Repository permissions**, set:

   | Permission | Level |
   | :--- | :--- |
   | Contents | Read & write |
   | Issues | Read |
   | Metadata | Read |
   | Pull requests | Read & write |

4. Under **Subscribe to events**, tick:
   - ✅ Pull request

5. Under **Where can this GitHub App be installed?**, choose:
   - **Only on this account** — if the app is for your personal or a single organisation
   - **Any account** — if you want to distribute the app to other users or organisations

6. Click **Create GitHub App**.

## Step 2: Generate a private key

After creating the app, you land on its settings page.

1. Scroll to **Private keys** and click **Generate a private key**.
2. A `.pem` file downloads automatically. Move it to a secure location — this file is the proof
   of identity for your GitHub App and cannot be recovered if lost.
3. Record the **App ID** shown at the top of the page.

## Step 3: Install the app on your repositories

1. In the app's settings sidebar, click **Install App**.
2. Click **Install** next to your account or organisation.
3. Choose which repositories the app can access:
   - **All repositories** — easier, but grants access to every current and future repository
   - **Only select repositories** — recommended; choose the specific repositories you want
     Release Regent to manage
4. Click **Install**.

## Step 4: Configure the server

Provide the App ID, private key, and webhook secret to the server as environment variables:

```bash
export GITHUB_APP_ID=<your-app-id>
export GITHUB_PRIVATE_KEY="$(cat /path/to/my-app.private-key.pem)"
export GITHUB_WEBHOOK_SECRET=<your-webhook-secret>
```

See [Deploy the server](install-server.md) for how to pass these variables securely in Docker,
Docker Compose, and Kubernetes.

## Step 5: Update the webhook URL

After your server is running and reachable from the internet:

1. Go back to your GitHub App settings.
2. Under **Webhook URL**, enter the public URL of your server with the `/webhook` path, for
   example: `https://release-regent.example.com/webhook`
3. Click **Save changes**.

GitHub will immediately send a `ping` event to verify the endpoint. Check your server logs for
a `200 OK` response.

## Rotating the private key

If a private key is compromised or expired:

1. Go to the app settings → **Private keys** → **Generate a private key**.
2. Download the new key.
3. Update `GITHUB_PRIVATE_KEY` in your server deployment with the new certificate contents.
4. Restart the server.
5. Delete the old key from the GitHub App settings once the server is confirmed healthy.

!!! danger "Keep the private key out of version control"
    Never commit the `.pem` file to a repository, even a private one. Use a secrets manager
    (AWS Secrets Manager, Azure Key Vault, HashiCorp Vault, or Kubernetes secrets) in
    production.

---

## Next steps

- [Deploy the server](install-server.md)
- [GitHub App authentication explained](../../explanation/github-app-model.md)
