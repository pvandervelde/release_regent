# GitHub App Setup Guide

This guide walks you through setting up a GitHub App for Release Regent, including webhook configuration and authentication.

## Overview

Release Regent uses a GitHub App for:

- **Webhook Processing**: Receiving notifications when PRs are merged
- **API Access**: Creating release PRs and managing repositories
- **Authentication**: JWT-based authentication with installation tokens

## Prerequisites

- GitHub repository with admin access
- Azure account for hosting the webhook endpoint
- Basic understanding of GitHub Apps and webhooks

## Step 1: Create GitHub App

### 1.1 Navigate to GitHub App Settings

1. Go to your GitHub account settings
2. Navigate to **Developer settings** → **GitHub Apps**
3. Click **New GitHub App**

### 1.2 Configure Basic Information

**App Name**: `Release Regent - [Your Org]`
**Description**: `Automated semantic versioning and changelog generation`
**Homepage URL**: `https://github.com/pvandervelde/release_regent`

### 1.3 Configure Webhook

**Webhook URL**: `https://your-function-app.azurewebsites.net/api/webhook`
**Webhook Secret**: Generate a secure random string (save this!)

```bash
# Generate webhook secret
openssl rand -hex 32
```

### 1.4 Configure Permissions

**Repository Permissions**:

- **Contents**: Read & Write (for creating releases and tags)
- **Metadata**: Read (for repository information)
- **Pull requests**: Read & Write (for managing release PRs)
- **Issues**: Read (for linking to issues in changelogs)

**Account Permissions**:

- **Email addresses**: Read (for commit author information)

### 1.5 Configure Events

Subscribe to these webhook events:

- ✅ **Pull request**: For detecting merged PRs
- ✅ **Push**: For detecting direct pushes to main branch (future)
- ⬜ **Release**: For release event handling (future)

### 1.6 Installation Settings

**Where can this GitHub App be installed?**

- Choose "Only on this account" for private use
- Choose "Any account" if you want to distribute the app

## Step 2: Generate Private Key

1. After creating the app, scroll down to **Private keys**
2. Click **Generate a private key**
3. Download the `.pem` file and store it securely
4. Note the **App ID** (displayed at the top of the app settings)

## Step 3: Install the GitHub App

### 3.1 Install on Repositories

1. Go to the **Install App** tab
2. Click **Install** next to your account
3. Choose repositories:
   - **All repositories** (easier but less secure)
   - **Selected repositories** (recommended - choose specific repos)

### 3.2 Note Installation ID

After installation, you'll see an installation ID in the URL:
`https://github.com/settings/installations/12345678`

The number `12345678` is your installation ID.

## Step 4: Configure Environment Variables

### 4.1 Azure Function Environment

Set these environment variables in your Azure Function:

```bash
# GitHub App Configuration
GITHUB_APP_ID=123456
GITHUB_APP_PRIVATE_KEY="-----BEGIN RSA PRIVATE KEY-----
MIIEpAIBAAKCAQEA...
-----END RSA PRIVATE KEY-----"

# Webhook Configuration
WEBHOOK_SECRET=your-generated-webhook-secret

# Optional: GitHub Enterprise
GITHUB_API_URL=https://api.github.com  # Default
GITHUB_WEB_URL=https://github.com      # Default
```

### 4.2 Local Development

Create a `.env` file for local testing:

```bash
# .env
GITHUB_APP_ID=123456
GITHUB_APP_PRIVATE_KEY_PATH=./path/to/private-key.pem
WEBHOOK_SECRET=your-webhook-secret
RUST_LOG=release_regent=debug
```

## Step 5: Test the Setup

### 5.1 Verify Authentication

```bash
# Test GitHub App authentication
cargo run --bin rr auth test
```

### 5.2 Test Webhook Endpoint

```bash
# Test webhook processing locally
cargo run --bin rr webhook test --payload sample-webhook.json
```

### 5.3 End-to-End Test

1. Create a test PR in your repository
2. Merge the PR
3. Check Azure Function logs for webhook processing
4. Verify no errors in the logs

## Configuration Examples

### release-regent.toml

```toml
[github]
app_id = 123456
private_key_path = "private-key.pem"
api_url = "https://api.github.com"
web_url = "https://github.com"

[webhook]
secret = "your-webhook-secret"
validate_signatures = true

[versioning]
prefix = "v"
allow_prerelease = true

[changelog]
use_git_cliff = true
include_authors = true
include_shas = true
include_links = true
```

### Azure Function local.settings.json

```json
{
  "IsEncrypted": false,
  "Values": {
    "AzureWebJobsStorage": "UseDevelopmentStorage=true",
    "FUNCTIONS_WORKER_RUNTIME": "custom",
    "GITHUB_APP_ID": "123456",
    "GITHUB_APP_PRIVATE_KEY": "-----BEGIN RSA PRIVATE KEY-----\\n...\\n-----END RSA PRIVATE KEY-----",
    "WEBHOOK_SECRET": "your-webhook-secret",
    "RUST_LOG": "release_regent=info"
  }
}
```

## Security Best Practices

### Private Key Security

❌ **Don't**:

- Commit private keys to version control
- Store private keys in plain text files
- Share private keys in chat or email

✅ **Do**:

- Use Azure Key Vault for production
- Use environment variables for development
- Rotate keys regularly

### Webhook Security

❌ **Don't**:

- Skip signature validation
- Use weak webhook secrets
- Expose webhook endpoints without authentication

✅ **Do**:

- Always validate webhook signatures
- Use strong, randomly generated secrets
- Monitor webhook processing logs

### Access Control

❌ **Don't**:

- Grant more permissions than necessary
- Install on all repositories unless needed
- Share installation tokens

✅ **Do**:

- Follow principle of least privilege
- Regularly review app permissions
- Monitor app usage and access

## Troubleshooting

### Common Issues

**"Bad credentials" error**:

- Check that App ID is correct
- Verify private key format (PEM with newlines)
- Ensure app is installed on the repository

**Webhook not received**:

- Check webhook URL is accessible from internet
- Verify webhook secret matches configuration
- Check firewall and network settings

**Signature validation fails**:

- Verify webhook secret is identical in GitHub and app
- Check for trailing whitespace in secret
- Ensure payload is not modified by middleware

### Debug Steps

1. **Check App Configuration**:

```bash
curl -H "Authorization: Bearer JWT_TOKEN" \
     -H "Accept: application/vnd.github.v3+json" \
     https://api.github.com/app
```

2. **Test Installation**:

```bash
curl -H "Authorization: Bearer JWT_TOKEN" \
     -H "Accept: application/vnd.github.v3+json" \
     https://api.github.com/app/installations
```

3. **Validate Webhook**:

```bash
# Send test webhook
cargo run --bin rr webhook send-test --repo owner/repo
```

### Logs and Monitoring

Enable detailed logging:

```bash
# Azure Function logs
func logs --function-name webhook-handler

# Local development
RUST_LOG=release_regent=debug cargo run
```

Monitor these metrics:

- Webhook delivery success rate
- Authentication token refresh rate
- API rate limit usage
- Error patterns and frequencies

## Next Steps

After successful setup:

1. **Configure Release Management**: Set up release PR templates
2. **Customize Changelogs**: Configure changelog generation
3. **Set Up Monitoring**: Add logging and alerting
4. **Test Edge Cases**: Test with various PR scenarios
5. **Document Process**: Create team documentation

## Support

For issues with Release Regent:

- Check the [GitHub repository](https://github.com/pvandervelde/release_regent)
- Review the [troubleshooting guide](./troubleshooting.md)
- Open an issue with detailed logs and configuration
