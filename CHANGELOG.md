### feat

- **release-pr**: feat(release-pr): use body_template for PR description [d62c6342281b37069e2c7eb2018255584f96be8b]

### fix

- **changelog**: fix(changelog): preserve history, add external strategy, wire config field [6bde5bd2485e391b8610d3f9cbf546c62b9a41a2]
- **config-provider**: fix(config-provider): add missing changelog field to ReleaseRegentConfig initializers [c716bb7edecadbed06b6bbf098c5078eacf40f05]
- **changelog**: fix(changelog): address PR review comments [3cd880ca2bffa56f7838fb0748a2f55d05b450e7]
- **changelog**: fix(changelog): source ConventionalCommit from analyzed_commits [af868309a9cf3237c5b0793620bb784d83edb782]
- **github_client**: fix(github_client): retry installation lookup on transient 401 JWT rejection [a473fd036b11c5d31c2111cc1b16be39ae5161cf]
- **changelog**: fix(changelog): filter non-conventional commits instead of requiring all be conventional [e3e47ebb3de389679862dc8321f985da1c2cfb05]
- **changelog**: fix(changelog): fix empty PR body when git-cliff filters all commits [1ed7ec5adfbd599287fe105a39dada2f31c0cefe]
- **core**: fix(core): address PR review findings [6a1b1a9e5b73951be297c70517cd22c67a74d6ff]
- **changelog**: fix(changelog): apply empty-output sentinel to External strategy [179e64b15695fe507c16ea1dbb04a563ca5f92e1]
- **changelog**: fix(changelog): preserve history, support external strategy, and scope PR body to current release (#131) [ad142cb77cbf48b3572a47ade4d8cf9c67b485a4]