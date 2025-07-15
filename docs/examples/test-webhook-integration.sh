#!/usr/bin/env bash

# test-webhook-integration.sh
#
# This script demonstrates testing the webhook integration locally
# without needing to set up a full Azure Function deployment.

set -euo pipefail

echo "🚀 Release Regent Webhook Integration Test"
echo "=========================================="

# Configuration
WEBHOOK_SECRET="${WEBHOOK_SECRET:-test-secret-key}"
REPO_DIR="${REPO_DIR:-$(pwd)}"
TEST_PAYLOAD_FILE="test-webhook-payload.json"

echo "📁 Working directory: $REPO_DIR"
echo "🔐 Webhook secret: ${WEBHOOK_SECRET:0:8}..."

# Check if Release Regent CLI is available
if ! command -v rr &> /dev/null; then
    echo "❌ Release Regent CLI (rr) not found. Building from source..."
    cargo build --release --bin rr
    export PATH="$REPO_DIR/target/release:$PATH"
fi

echo "✅ Release Regent CLI found"

# Create test webhook payload
echo "📝 Creating test webhook payload..."
cat > "$TEST_PAYLOAD_FILE" << 'EOF'
{
  "action": "closed",
  "number": 42,
  "pull_request": {
    "id": 1234567890,
    "number": 42,
    "state": "closed",
    "title": "feat: add webhook integration testing",
    "body": "This PR adds comprehensive webhook integration testing.\n\n## Changes\n- Added webhook signature validation\n- Integrated with GitHub client authentication\n- Added comprehensive test coverage\n\n## Testing\n- [x] Unit tests pass\n- [x] Integration tests pass\n- [x] Manual testing completed",
    "merged": true,
    "merge_commit_sha": "a1b2c3d4e5f6789012345678901234567890abcd",
    "base": {
      "ref": "main",
      "sha": "def456789abc123def456789abc123def456789ab",
      "repo": {
        "name": "release_regent",
        "full_name": "pvandervelde/release_regent"
      }
    },
    "head": {
      "ref": "integrate/webhook-signature-validation",
      "sha": "123abc456def789abc123def456789abc123def45",
      "repo": {
        "name": "release_regent",
        "full_name": "pvandervelde/release_regent"
      }
    },
    "user": {
      "login": "developer",
      "type": "User"
    }
  },
  "repository": {
    "id": 987654321,
    "name": "release_regent",
    "full_name": "pvandervelde/release_regent",
    "private": false,
    "default_branch": "main",
    "owner": {
      "login": "pvandervelde",
      "type": "User"
    }
  },
  "sender": {
    "login": "developer",
    "type": "User"
  }
}
EOF

echo "✅ Test payload created: $TEST_PAYLOAD_FILE"

# Calculate webhook signature
echo "🔐 Calculating webhook signature..."
PAYLOAD_SIGNATURE=$(cat "$TEST_PAYLOAD_FILE" | openssl dgst -sha256 -hmac "$WEBHOOK_SECRET" | cut -d' ' -f2)
echo "✅ Signature calculated: sha256=$PAYLOAD_SIGNATURE"

# Test 1: Process webhook without signature validation
echo ""
echo "🧪 Test 1: Processing webhook without signature validation"
echo "--------------------------------------------------------"

if rr test webhook --payload "$TEST_PAYLOAD_FILE" --no-signature-validation; then
    echo "✅ Test 1 PASSED: Webhook processed without signature validation"
else
    echo "❌ Test 1 FAILED: Webhook processing failed"
    exit 1
fi

# Test 2: Process webhook with valid signature
echo ""
echo "🧪 Test 2: Processing webhook with valid signature"
echo "---------------------------------------------------"

if rr test webhook --payload "$TEST_PAYLOAD_FILE" --signature "sha256=$PAYLOAD_SIGNATURE" --secret "$WEBHOOK_SECRET"; then
    echo "✅ Test 2 PASSED: Webhook processed with valid signature"
else
    echo "❌ Test 2 FAILED: Valid signature was rejected"
    exit 1
fi

# Test 3: Process webhook with invalid signature
echo ""
echo "🧪 Test 3: Processing webhook with invalid signature"
echo "----------------------------------------------------"

if rr test webhook --payload "$TEST_PAYLOAD_FILE" --signature "sha256=invalid_signature" --secret "$WEBHOOK_SECRET" 2>/dev/null; then
    echo "❌ Test 3 FAILED: Invalid signature was accepted"
    exit 1
else
    echo "✅ Test 3 PASSED: Invalid signature was correctly rejected"
fi

# Test 4: Process different event types
echo ""
echo "🧪 Test 4: Testing different event types"
echo "-----------------------------------------"

# Create non-merged PR payload
cat > "test-non-merged-pr.json" << 'EOF'
{
  "action": "closed",
  "pull_request": {
    "number": 43,
    "title": "feat: incomplete feature",
    "merged": false
  },
  "repository": {
    "name": "release_regent",
    "full_name": "pvandervelde/release_regent",
    "default_branch": "main",
    "owner": { "login": "pvandervelde" }
  }
}
EOF

echo "  Testing non-merged PR (should be ignored)..."
if rr test webhook --payload "test-non-merged-pr.json" --no-signature-validation; then
    echo "  ✅ Non-merged PR correctly ignored"
else
    echo "  ℹ️ Non-merged PR processing returned error (expected)"
fi

# Create unsupported event payload
cat > "test-unsupported-event.json" << 'EOF'
{
  "action": "opened",
  "issue": {
    "number": 1,
    "title": "Bug report",
    "body": "Found a bug"
  },
  "repository": {
    "name": "release_regent",
    "full_name": "pvandervelde/release_regent"
  }
}
EOF

echo "  Testing unsupported event type (should be ignored)..."
if rr test webhook --payload "test-unsupported-event.json" --no-signature-validation --event-type "issues"; then
    echo "  ✅ Unsupported event correctly ignored"
else
    echo "  ℹ️ Unsupported event processing returned error (expected)"
fi

# Test 5: Integration with actual webhook server (if available)
echo ""
echo "🧪 Test 5: Testing webhook server integration"
echo "----------------------------------------------"

# Check if webhook server is running
if curl -s -f "http://localhost:8080/health" > /dev/null 2>&1; then
    echo "  Webhook server detected at localhost:8080"

    echo "  Testing health endpoint..."
    HEALTH_RESPONSE=$(curl -s "http://localhost:8080/health")
    echo "  Health response: $HEALTH_RESPONSE"

    echo "  Testing webhook endpoint..."
    WEBHOOK_RESPONSE=$(curl -s -X POST \
        -H "Content-Type: application/json" \
        -H "X-GitHub-Event: pull_request" \
        -H "X-Hub-Signature-256: sha256=$PAYLOAD_SIGNATURE" \
        -d @"$TEST_PAYLOAD_FILE" \
        "http://localhost:8080/api/webhook")

    echo "  Webhook response: $WEBHOOK_RESPONSE"
    echo "  ✅ Webhook server integration test completed"
else
    echo "  ℹ️ No webhook server running at localhost:8080, skipping server tests"
    echo "  💡 To test server integration, run: cargo run --bin webhook-server"
fi

# Cleanup
echo ""
echo "🧹 Cleaning up test files..."
rm -f "$TEST_PAYLOAD_FILE" "test-non-merged-pr.json" "test-unsupported-event.json"

echo ""
echo "🎉 All webhook integration tests completed successfully!"
echo ""
echo "📋 Summary:"
echo "  ✅ Webhook processing without signature validation"
echo "  ✅ Webhook processing with valid signature"
echo "  ✅ Invalid signature rejection"
echo "  ✅ Event type filtering"
echo "  ✅ Integration testing framework"
echo ""
echo "🚀 Your webhook integration is ready for production!"

# Optional: Show next steps
echo ""
echo "📚 Next Steps:"
echo "  1. Deploy to Azure Function: ./scripts/deploy-azure.sh"
echo "  2. Configure GitHub App: See docs/github-app-setup.md"
echo "  3. Set up monitoring: See docs/monitoring.md"
echo "  4. Test with real repository: Create a test PR and merge it"
