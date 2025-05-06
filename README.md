# Release_Regent

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## 👑 Overview

Release_Regent is a GitHub app that automates your release management process. It handles versioning, release pull requests, and GitHub releases - ruling over your software delivery process with precision and reliability.

## ✨ Features

- **Automated Release PR Creation**: When any PR is merged into the main branch, Release_Regent automatically creates or updates a release PR with the next version number
- **Versioning Management**: Intelligently determines the next semantic version based on commit messages and PR content
- **GitHub Release Automation**: When a release PR is merged into main, automatically creates a git tag and GitHub release
- **Customizable Configuration**: Adapt Release_Regent to your project's specific needs through simple configuration

## 🔄 How It Works

1. **PR Merge Detection**: Responds to GitHub webhook events when PRs are merged to your main branch
2. **Release PR Management**:
   - If no release PR exists, creates one with the next version
   - If a release PR already exists, updates it with new changes
3. **Release Creation**: When the release PR is merged, automatically:
   - Creates a git tag with the version
   - Generates a GitHub release with changelog notes
   - Publishes the release

Release_Regent runs as a serverless function (Azure Function or AWS Lambda) that responds to GitHub webhook events, requiring no dedicated server.

## 🛠️ Installation

Release_Regent is designed to run as a serverless function that responds to GitHub webhooks. You can deploy it using either Azure Functions or AWS Lambda.

### Azure Functions Deployment

```bash
# Clone the repository
git clone https://github.com/yourusername/release_regent.git
cd release_regent

# Install Azure Functions Core Tools
npm install -g azure-functions-core-tools@4

# Install dependencies
npm install

# Configure local settings
cp local.settings.example.json local.settings.json
# Edit local.settings.json with your GitHub App credentials and settings

# Deploy to Azure
func azure functionapp publish YourFunctionAppName
```

### AWS Lambda Deployment

```bash
# Clone the repository
git clone https://github.com/yourusername/release_regent.git
cd release_regent

# Install dependencies
npm install

# Configure AWS credentials
aws configure

# Deploy using Serverless Framework
npm install -g serverless
serverless deploy
```

## ⚙️ Configuration

### Application Configuration

For both Azure Functions and AWS Lambda, you'll need to configure environment variables:

**Azure Functions (`local.settings.json` for local development):**

```json
{
  "IsEncrypted": false,
  "Values": {
    "GITHUB_APP_ID": "your-github-app-id",
    "GITHUB_APP_PRIVATE_KEY": "your-private-key",
    "GITHUB_WEBHOOK_SECRET": "your-webhook-secret",
    "DEFAULT_CONFIG_PATH": ".github/release-regent.yml"
  }
}
```

**AWS Lambda (in `serverless.yml`):**

```yaml
provider:
  name: aws
  runtime: nodejs16.x
  environment:
    GITHUB_APP_ID: ${param:GITHUB_APP_ID}
    GITHUB_APP_PRIVATE_KEY: ${param:GITHUB_APP_PRIVATE_KEY}
    GITHUB_WEBHOOK_SECRET: ${param:GITHUB_WEBHOOK_SECRET}
    DEFAULT_CONFIG_PATH: ".github/release-regent.yml"
```

### Repository Configuration

Create a `.github/release-regent.yml` file in each repository where you want to use Release_Regent:

```yaml
# Basic configuration
version_prefix: "v"  # Optional prefix for version tags (e.g., v1.0.0)
branches:
  main: main  # Your main branch name

# Release PR settings
release_pr:
  title_template: "chore(release): prepare for version ${version}"
  body_template: |
    # Release ${version}

    ## What's Changed
    ${changelog}

    ## Installation
    ```
    npm install your-package@${version}
    ```

# Release settings
releases:
  draft: false
  prerelease: false
  generate_notes: true
```

## 🔗 Related Projects

Release_Regent is part of a suite of GitHub automation tools:

- [Merge_Warden](https://github.com/yourusername/merge_warden) - Checks PRs to ensure they match certain standards
- [Template_Teleporter](https://github.com/yourusername/template_teleporter) - Copies GitHub issue and PR templates from a master repository
- [Repo_Roller](https://github.com/yourusername/repo_roller) - Creates new repositories from template repositories

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## 📜 License

This project is licensed under the MIT License - see the LICENSE file for details.

## 📊 Project Status

Release_Regent is currently in active development.

## 🔮 Future Plans

- Integration with CI/CD pipelines
- Support for monorepo architectures
- Custom release notes formatting
- Advanced changelog generation
- Support for conventional commits
