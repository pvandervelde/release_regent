# Changelog

## [0.5.0] - 2026-05-18

### Features

- **automator**: add version_prefix to AutomatorConfig and helpers [1b3370ddbc9e23f6da9ed237cf13a3038629aa28]
- **core**: thread version_prefix from CoreConfig into orchestrator and automator [53433dcab73c516a10423ef3f4931cec59e9d3c1]
- **orchestrator**: add version_prefix field to OrchestratorConfig [10d9739f29c6859927c1fbdae5e36e3c52fe864d]
- **server**: propagate version_prefix through webhook handler [4c8b35171d7c042677ec9fa55ceec16c9f7158c8]


### Bug Fixes

- **changelog**: treat BrokenPipe as non-fatal on stdin write [0fb13325101843dc0441dcc17e5ae94c28895493]
- **core**: honour version_prefix config when creating tags and branches (#137) [ec8da5780a469df7c0b74f348541f808dc449f64]
- **core,server**: address PR review comments [2f48a58c41982937923278e6636fb04724cf7fb9]


### Documentation

- **server**: fix stale v references in main.rs and handler.rs [8479ddbdb7754f6884b312417a9545c0b173d54e]


### Tests

- update call sites and add version_prefix coverage [586ace968b04cbad0ea44f8c7dfb32fc0d65ddac]
