# Changelog

## [0.5.0] - 2026-06-26

### Features

- **release**: Add SBOM generation and attestations to release workflow [90d4c6fde6ab88fc65c81eb296378a9288335353]
- **release**: Add SBOM generation and attestations to release workflow (#160) [2e98e9566dd1edfc1e41c5e643e54e2430f3eb15]


### Bug Fixes

- **config**: address PR review comments on repo dotfile [b6b997ff9452cc5c4c42f1c0056cfff364f84c6f]
- **deps**: update quinn-proto to 0.11.15 (RUSTSEC-2026-0185) [45a1d151ae947ae9ac0681190aaf9cfd14091988]
- **orchestrator**: read manifests from base branch in create path; add regression test [dfe757c6f7a09b99ac8dd183d6cf3c4011f5c799]
- **orchestrator**: read manifests from base branch on release PR update [0c38a7e478900ab301ba59a51a6382a20e476077]
- **orchestrator**: read manifests from base branch on release PR update (#198) [57b896530436958927c0028d4fbdf7dcb17bdc01]
- **release**: Address PR review comments on SBOM/attestation workflow [d1ad7b23160ec3579f882eacc319344f67caa74e]
- **release**: Scope elevated permissions to job level and document multi-platform SBOM limitation [d375ad6b0dc751c1c38a92176b2c96eab43a60fa]


### Documentation

- add sample config files for all four configuration levels [843019a9ce5ff00c1c2c5833f28f0c1c42aaa85e]
- add sample config files for all four configuration levels (#197) [22d8eb3c12005eee1870576d724d90c155218f61]
- address PR review comments on configuration hierarchy docs [7d4fa05fc52e5ceec5a95a2c72ce098084a8b095]
- document per-org metadata repository and configuration hierarchy [b6bd24a7bd9242eea82072f0b0c39a994021436c]
- document per-org metadata repository and configuration hierarchy (#194) [67e2d6e28b8f45a22f435558f7c52237994c2ef2]
- fix filename context ambiguity and group warn! consistency [3ad896204711bd9b8efa762e2ad8a73ab2056de4]


### Tests

- **audit**: mutation + fuzz audit for task 5 parsing tests [432adfdc2acabf36748d43e94e4c2b11da764321]
- **core**: add property-based tests for parsing and validation logic [2a403d657ca9eab554ed0a12c47485368f03d875]
- **core**: add property-based tests, mutation kill tests, fuzz targets, and CI integration (#162) [1fd1b95c71fb17cb610b1103a6c7e72e49a2e04a]
- **core**: address PR review comments on test quality [1c2fd3996f75a76463bb1bb10cec4107d03f41e1]
- **fuzz**: add seed corpus and fix CI fuzz job [03770b395fcd3c63066e3d7c9329b3f751e81f1c]


### Continuous Integration

- **security**: implement supply chain security gate pattern [e397a8c45750bcb991ec1a607fda645e23275d12]
- **security**: implement supply chain security gate pattern (#164) [2b684514bc97fa7708504398360b56f5e604b51c]
- Remove unused workflows [512c72ce88cad3390616e380d451b5b4314fd1ad]
- Security scan only errors when issue is fixable [6d3469b1cb241af20e43067ce01f79b186cc3546]
- add fuzz-tests job to CI workflow [cc1b72b7a8c8f80f2ac6381f96a0f32b2460a9a0]
- improve the log for the security scans [c9f4658fdc3f23a8630a6a7c6370deb1c4d7b1ba]


### Chores

- **deps**: bump openssl from 0.10.75 to 0.10.80 [febca7bc69d584088631523ba37c50700a744411]
- **deps**: bump openssl from 0.10.75 to 0.10.80 (#163) [ce145ec370f7eb8d51a8ad917107537546200e1d]
- **deps**: pin dependencies [05bf4f032fd44f061784cca583efe5fbb23b888f]
- **deps**: pin dependencies (#154) [ce0cb43e659947370a286fd881d93a8d4836ad3e]
- **deps**: refresh cargo dependencies [4ea9b142f39dae1a67a20ba04191c6806e28691a]
- **deps**: refresh cargo dependencies (#176) [01276df6beca141b79f6db21f23f92078fe66f9d]
- **deps**: update Rust dependencies and Rust toolchain to 1.96 [dcdf8b8a50a986fb1f20c71b81bc58dc00e434b8]
- **deps**: update Rust dependencies and toolchain to 1.96 (#196) [5535e4dda5359e7e4d100344e255236a7faf9fd4]
- **deps**: update actions/checkout digest to 34e1148 [81e9f9976e2643a712db7f58c171625c8051a423]
- **deps**: update actions/checkout digest to 34e1148 (#155) [5e62d58786c5a13019d2f063d5de07b64a822ebc]
- **deps**: update actions/deploy-pages action to v5 [c1fcb20ff2520477d99d2845e5b38c6f8ab20b64]
- **deps**: update actions/deploy-pages action to v5 (#179) [7044970b2ecb5b96dd0e73cdc563b6aa8288327a]
- **deps**: update actions/setup-python action to v6 [09da82d85122c8113be4e59fc7a9cf3e3baf7df1]
- **deps**: update actions/setup-python action to v6 (#180) [0bd077952ab03a5501bb775817b106b9d9ec826c]
- **deps**: update actions/upload-pages-artifact action to v5 [255b89fa719c0fea76e38f6abfc51630516e96cb]
- **deps**: update actions/upload-pages-artifact action to v5 (#181) [5acabce8c2cd7ab22c75e495ddc853ed799110fc]
- **deps**: update anthropics/claude-code-action digest to 4d7e1f0 [dfa2063c9175cb11f050495fa79a59fd25083ea7]
- **deps**: update anthropics/claude-code-action digest to 4d7e1f0 (#156) [dd4d0aabbc8410b7382cde477353119f6ae08b39]
- **deps**: update azure-sdk-for-rust monorepo [6e0a7c6c7498d0774a310a2de22cbbaef02217a3]
- **deps**: update azure-sdk-for-rust monorepo (#32) [42f568e92e12e179a2ca0265d99efd480d7d90b3]
- **deps**: update codecov/codecov-action action to v6 [2040ee0de46dae2a936484465714418fc3092c54]
- **deps**: update codecov/codecov-action action to v6 (#173) [6425d363b945376d5e55f4b4479fd351f4dc027a]
- **deps**: update codecov/codecov-action action to v7 [270f24b03cae02b3d438eb3c3b81f2a48e2e959a]
- **deps**: update codecov/codecov-action action to v7 (#182) [53a37ad62045531a1ccd6d6c789863b67be393bc]
- **deps**: update codecov/codecov-action digest to 0fb7174 [9e95cf2ee70ac66c46866c345e33e2f2904cb3ae]
- **deps**: update codecov/codecov-action digest to 0fb7174 (#157) [8753e00df2cb208825fa3e806a8cb5ab0f4f146d]
- **deps**: update docker/build-push-action action to v7 [27807b03ff8cacf4cca4270483c6f2207785c938]
- **deps**: update docker/build-push-action action to v7 (#191) [372bc8d958d9651bd04a4cd10f03796bda2a046e]
- **deps**: update docker/login-action action to v4 [2a9c74ebb562f631c083329cb927beb5a4a89a73]
- **deps**: update docker/login-action action to v4 (#192) [06353027c86076f807997473d1bf66662c9c54b4]
- **deps**: update docker/metadata-action action to v6 [cb34a199ef7cd61878970c040c08cce954b977d1]
- **deps**: update docker/metadata-action action to v6 (#193) [bf2ef626b4f7c9df24f20231985c1451287de373]
- **deps**: update rust crate dirs to v6 [ed6d7e6f83006fc2396969c9326291df3c1a8fcd]
- **deps**: update rust crate dirs to v6 (#63) [8447d3d42eff2e7feba497a2f816b4aed6a87c75]
- **deps**: update rust crate hmac to 0.13 [915b7974615f90391082ad495513af11fed64c65]
- **deps**: update rust crate hmac to 0.13 (#170) [ec26d473cc316466f05ff832ebcf694a2644347d]
- **deps**: update rust crate jsonschema to 0.46 [7993a845955817e8c20bd9acdc9ed6a6804b6289]
- **deps**: update rust crate jsonschema to 0.46 (#62) [6588a438e02002039f5e8b796605d1e0799ff1d7]
- **deps**: update rust crate jsonwebtoken to v10 [security] [b058da65db471c28549ab662f49ac4d05637eb45]
- **deps**: update rust crate jsonwebtoken to v10 [security] (#151) [919cf67ebfd92f4e0dc7bdc7686996321f97e32c]
- **deps**: update rust crate libfuzzer-sys to v0.4.13 [0da876036a9374b917bf7a97ac41e5db67b0366d]
- **deps**: update rust crate libfuzzer-sys to v0.4.13 (#172) [2f49faa9212a39014ecca32feb503b744e0d6d35]
- **deps**: update rust crate mockall to 0.14 [3c372861696e58865298017dae4348164b46384b]
- **deps**: update rust crate mockall to 0.14 (#171) [c90f2a663dfb8750ead7741c2e7ccbb4eadd3b72]
- **deps**: update rust crate rand to v0.9.3 [security] [d56d514e6842b66c7a7d3aa7efeaa4e52cc8e0a2]
- **deps**: update rust crate rand to v0.9.3 [security] (#150) [e4ac050745f1260eb010abbf16e0809a62015f05]
- **deps**: update rust crate toml to 0.9 [157c8163cc09f810ca001b377254412604a91af0]
- **deps**: update rust crate toml to 0.9 (#35) [6e3e8f051584a3815f85ba25a8178424203d0597]
- **renovate**: Improve the renovate config [9ada3811b3efa97850d53a63b0483dd18e02252a]
- **renovate**: Improve the renovate config (#175) [4c7ff765db9701fdb1e8ead508958be819b7f323]
- Add merge-warden config [190a0408e921135b0adf573382cc4df06b17699c]
- Add merge-warden config (#153) [345036ef3aedd1a067c7fd3d8c8b3f9dd4196e4b]
- Add release regent config [887d04b966244dd33bebd68c7cbb4ab115cec52b]
- Add release regent config (#152) [5f8f6f7ddfff5bfba3f15a0d9ca0a7af3360c223]
- Address PR comments [c3ebef0f27fb9a7a399de42d94c19b2b24f5cee2]
- Fix compiler errors due to github-bot-sdk upgrade [8dc047c7ae047f94a3ba76ffc3b6a5dc7d6d8569]
- Fix issue in renovate config [52f9995eb60ae6cbe9799a3951efd0ab1aa41096]
- Fix issue in renovate config (#149) [dabd210fb4c62e154edc6506b20f2d5552359874]
- Fix size label in merge-warden config [016dc6cb5d3f51811d2f3f969867365eeb8c65c1]
- Fix size label in merge-warden config (#159) [7ad5b6c205971f56ddd2f2bb4b3f711b7bc79941]
- Update dependencies [521f7c2ea7d3a7ea6e7c0f5b6c0a525779469b58]
- Update renovate to only update once a week [5d028e78b8730bbd7f93ebc80a06528d1188cf61]
- Update renovate to only update once a week (#177) [e5b04072194a70a67b0110d2f3caae87e06a070a]
