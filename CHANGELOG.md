# Changelog

## [0.1.3](https://github.com/sotayamashita/todo-scan/compare/v0.1.2...v0.1.3) (2026-02-24)


### Bug Fixes

* **ci:** apply cargo fmt and raise TODO max to 90 ([ed9078a](https://github.com/sotayamashita/todo-scan/commit/ed9078af400e87ca9413d34238efd41ba173720c))

## [0.1.2](https://github.com/sotayamashita/todo-scan/compare/v0.1.1...v0.1.2) (2026-02-23)


### Bug Fixes

* **release:** resolve release-please and cargo-dist GitHub Release conflict ([3473572](https://github.com/sotayamashita/todo-scan/commit/347357234dd9c9e3d0cdacd999b131e43cc08e6a))

## [0.1.1](https://github.com/sotayamashita/todo-scan/compare/v0.1.0...v0.1.1) (2026-02-23)


### Bug Fixes

* **release:** skip GitHub release in release-please to avoid conflict with cargo-dist ([#156](https://github.com/sotayamashita/todo-scan/issues/156)) ([0cd1966](https://github.com/sotayamashita/todo-scan/commit/0cd1966d1f846f27b9abc9cea30d598868ddb2ce))

## 0.1.0 (2026-02-23)


### Features

* **action:** add composite GitHub Action for CI integration ([b919fa5](https://github.com/sotayamashita/todo-scan/commit/b919fa59d90a747ea9f8b0f3d03b4b1f503ca298)), closes [#105](https://github.com/sotayamashita/todo-scan/issues/105)
* **blame:** add blame subcommand for TODO ownership tracking ([#5](https://github.com/sotayamashita/todo-scan/issues/5)) ([46a82e2](https://github.com/sotayamashita/todo-scan/commit/46a82e23023aa755ea0e749989d02322396f742f))
* **check:** add --expired flag for TODO deadline enforcement ([#4](https://github.com/sotayamashita/todo-scan/issues/4)) ([261607d](https://github.com/sotayamashita/todo-scan/commit/261607d9beab30e55bb1618053d50c3927e0a377))
* **clean:** add clean subcommand for stale TODO detection ([#31](https://github.com/sotayamashita/todo-scan/issues/31)) ([b93ad98](https://github.com/sotayamashita/todo-scan/commit/b93ad98b41cfd39ac27d6fa6d8b98048559e6f51))
* **cli:** add --detail minimal|normal|full progressive disclosure flag ([#98](https://github.com/sotayamashita/todo-scan/issues/98)) ([abbd5cb](https://github.com/sotayamashita/todo-scan/commit/abbd5cbed37981a8736842f4cfa5d6cbb0955d36))
* **cli:** add brief compressed summary command ([#101](https://github.com/sotayamashita/todo-scan/issues/101)) ([86544ee](https://github.com/sotayamashita/todo-scan/commit/86544ee135c6449268e4e7857bbed5cc7f5e6923))
* **cli:** add init and completions subcommands ([#8](https://github.com/sotayamashita/todo-scan/issues/8)) ([7d1333e](https://github.com/sotayamashita/todo-scan/commit/7d1333e4aa2644ec79b8e5a2514c9ec36ccc4ef5))
* **context:** add context subcommand and -C flag ([#22](https://github.com/sotayamashita/todo-scan/issues/22)) ([322894e](https://github.com/sotayamashita/todo-scan/commit/322894eafdf074575d5ca9e4cb140faa3ed07701))
* implement todox MVP with list, diff, and check commands ([1091d47](https://github.com/sotayamashita/todo-scan/commit/1091d4794830e488f3514fa93d1f51302fe4f146))
* **lint:** add lint subcommand for TODO formatting ([#23](https://github.com/sotayamashita/todo-scan/issues/23)) ([6b8e326](https://github.com/sotayamashita/todo-scan/commit/6b8e3268b3180d466292e5c652a1b8d3250e98ee))
* **list:** add group-by and filter flags ([#21](https://github.com/sotayamashita/todo-scan/issues/21)) ([deb5c8b](https://github.com/sotayamashita/todo-scan/commit/deb5c8bcf4b3ad040b9acd2f596d2b26966477fe))
* **output:** add github-actions, sarif, markdown formats ([#6](https://github.com/sotayamashita/todo-scan/issues/6)) ([da150d4](https://github.com/sotayamashita/todo-scan/commit/da150d41b6d117c2f45011f180cc0eca48d8f5d7))
* **output:** expose stable TODO IDs in all output formats ([#99](https://github.com/sotayamashita/todo-scan/issues/99)) ([f6779c6](https://github.com/sotayamashita/todo-scan/commit/f6779c66dedd3baaea1e32071772650810c383c0))
* **relate:** add relate subcommand for TODO relationship discovery ([#30](https://github.com/sotayamashita/todo-scan/issues/30)) ([574e06c](https://github.com/sotayamashita/todo-scan/commit/574e06ce36ca5aab30687b9d7c6e14c55cc20f8b))
* **report:** add HTML technical debt dashboard ([#26](https://github.com/sotayamashita/todo-scan/issues/26)) ([239d484](https://github.com/sotayamashita/todo-scan/commit/239d484f7a8c2b71aa1629ec48bb5e025125769d))
* **scanner:** add todox:ignore inline suppression comment ([#75](https://github.com/sotayamashita/todo-scan/issues/75)) ([ce14170](https://github.com/sotayamashita/todo-scan/commit/ce14170a0b27d83612cd92197d1f154efefc21f7))
* **search:** add search subcommand for TODOs ([#20](https://github.com/sotayamashita/todo-scan/issues/20)) ([9eaa05f](https://github.com/sotayamashita/todo-scan/commit/9eaa05ff2f52665989cb13594b4b332d470f1f02))
* **stats:** add stats subcommand with dashboard summary ([#7](https://github.com/sotayamashita/todo-scan/issues/7)) ([f5415fc](https://github.com/sotayamashita/todo-scan/commit/f5415fc663d6f31217db0fe8f3da1de7428cdd1a))
* **tasks:** add tasks subcommand for Claude Code export ([#32](https://github.com/sotayamashita/todo-scan/issues/32)) ([2389450](https://github.com/sotayamashita/todo-scan/commit/23894506be9272edc05d7ffc0e649cab17d4945c))
* **watch:** add watch subcommand for live TODO monitoring ([#9](https://github.com/sotayamashita/todo-scan/issues/9)) ([f01cbae](https://github.com/sotayamashita/todo-scan/commit/f01cbae9b8922077b066207ca2066d07a7470dfb))
* **workspace:** add monorepo/workspace detection and per-package scanning ([#27](https://github.com/sotayamashita/todo-scan/issues/27)) ([e430842](https://github.com/sotayamashita/todo-scan/commit/e430842090961d0fa18b5263b50f1dd0d1233822))


### Bug Fixes

* **action:** skip install when todo-scan is already on PATH ([db52dd7](https://github.com/sotayamashita/todo-scan/commit/db52dd79860e9f8763184de78bab14d8404de69f))
* address code review findings (7 issues) ([8bc855e](https://github.com/sotayamashita/todo-scan/commit/8bc855ec913ec994729dd3c7cc6bfde8a183f0ed))
* **blame:** remove redundant field name for clippy ([68244a0](https://github.com/sotayamashita/todo-scan/commit/68244a0920bc5d74a128b13352decd3dac0db340))
* **ci:** raise TODO gate threshold to 105 ([acfe963](https://github.com/sotayamashita/todo-scan/commit/acfe9632daf18dbbc9759e5653058bee1f9aa8d0))
* **ci:** raise TODO gate threshold to 105 ([ba18119](https://github.com/sotayamashita/todo-scan/commit/ba181190917339d57028ec3d973befb34636f6bd))
* **ci:** use full git history for todox diff ([1203074](https://github.com/sotayamashita/todo-scan/commit/12030741d97f21e79e08661c1856047374b5e2ca))
* **clean:** resolve clippy warnings for redundant closure, cast, and map iteration ([40c31ea](https://github.com/sotayamashita/todo-scan/commit/40c31ea9a32eded092564d77e0d58e42b2a9f279))
* **date_utils:** use inner doc comment for module-level docs ([#96](https://github.com/sotayamashita/todo-scan/issues/96)) ([7d1264d](https://github.com/sotayamashita/todo-scan/commit/7d1264d32605e82560ba4ca95e11eaa9c7a3e84f))
* **deps:** update notify to v8 and notify-debouncer-mini to v0.7 ([271def9](https://github.com/sotayamashita/todo-scan/commit/271def9ce84cece7ecf9fa7f6a4d1d0ceeda4e3f)), closes [#151](https://github.com/sotayamashita/todo-scan/issues/151) [#144](https://github.com/sotayamashita/todo-scan/issues/144)
* **deps:** update rust crate colored to v3 ([a3e3560](https://github.com/sotayamashita/todo-scan/commit/a3e3560b4fa458d4495ec2e60588faa97dfdc25e))
* **deps:** update rust crate dialoguer to 0.12 ([#143](https://github.com/sotayamashita/todo-scan/issues/143)) ([b49334b](https://github.com/sotayamashita/todo-scan/commit/b49334b1e9157213e8fa758b05d59078e51ec6d2))
* **deps:** update rust crate schemars to 0.9 ([#145](https://github.com/sotayamashita/todo-scan/issues/145)) ([e35e4d0](https://github.com/sotayamashita/todo-scan/commit/e35e4d0595d88375d0d17735e5b31dbd9537a2ef))
* **deps:** update rust crate schemars to v1 ([#152](https://github.com/sotayamashita/todo-scan/issues/152)) ([fa26662](https://github.com/sotayamashita/todo-scan/commit/fa26662ae8eabc20b5aab9dcb5aa2d6745b40dc0))
* **deps:** update rust crate thiserror to v2 ([4031e3d](https://github.com/sotayamashita/todo-scan/commit/4031e3d69bf756c691e3016ce2ace97269aa3770))
* **deps:** update rust crate toml to 0.9 ([8290657](https://github.com/sotayamashita/todo-scan/commit/829065785dedb16333776a9313a067039fcd4c3a))
* **deps:** update rust crate toml to v1 ([a9ae455](https://github.com/sotayamashita/todo-scan/commit/a9ae4553c71f9da198448610141b64f549692ed3))
* **deps:** update rust crate toml_edit to 0.25 ([#146](https://github.com/sotayamashita/todo-scan/issues/146)) ([f4def16](https://github.com/sotayamashita/todo-scan/commit/f4def169d427174c49bf6fb3039dd186f31eb0a0))
* **model:** make GoWork serialize to "go" for JSON/Display consistency ([#27](https://github.com/sotayamashita/todo-scan/issues/27)) ([307a841](https://github.com/sotayamashita/todo-scan/commit/307a84129cd217da535b8fa1e3b0d10c37908d71))
* **plugin:** update repository URLs from todox to todo-scan ([a0299b6](https://github.com/sotayamashita/todo-scan/commit/a0299b6359b0ef2f7eebba14f047b6446b85bcdc))
* **relate:** resolve clippy warnings for CI compliance ([#30](https://github.com/sotayamashita/todo-scan/issues/30)) ([cc8c1d6](https://github.com/sotayamashita/todo-scan/commit/cc8c1d618ba1cc2266d0ff76f9c7d9a26b5edf8c))
* **renovate:** ignore bincode v2+ (v3 is intentionally broken) ([c2d5af9](https://github.com/sotayamashita/todo-scan/commit/c2d5af9d6675246ca5f6037b668eccec774bf8fd))
* **scanner:** add quote-awareness to is_in_comment ([#74](https://github.com/sotayamashita/todo-scan/issues/74)) ([ac20f01](https://github.com/sotayamashita/todo-scan/commit/ac20f01e12bd8358d1fec711dca7d5cf092c43d2))
* **scanner:** add word boundary after tag in regex ([#72](https://github.com/sotayamashita/todo-scan/issues/72)) ([cbdfbbf](https://github.com/sotayamashita/todo-scan/commit/cbdfbbf6ead845b373a33dd5babb9fe54e012eea))
* **scanner:** reduce regex false positives by requiring comment context ([3dacfef](https://github.com/sotayamashita/todo-scan/commit/3dacfefa4fe82e35dd082e39488b1fb37a07513d))
* **scanner:** reduce regex false positives by requiring comment context ([fdf485e](https://github.com/sotayamashita/todo-scan/commit/fdf485ea6d1845fddc0d7a5f65dcd30dbdffc300)), closes [#1](https://github.com/sotayamashita/todo-scan/issues/1)
* **search:** replace map_or with is_some_and for clippy ([a3b155a](https://github.com/sotayamashita/todo-scan/commit/a3b155ab06176abdff9ec36f181bcea69baa1747))
* **security:** add file size limit to prevent OOM on large files ([e33bc93](https://github.com/sotayamashita/todo-scan/commit/e33bc93f873b9f1e3d2427d2a5cbfecb67c8125c))
* **security:** add size limit to cache deserialization ([d3de98e](https://github.com/sotayamashita/todo-scan/commit/d3de98ebf9b1be99bdbff655af2b116f837a37c4))
* **security:** complete Markdown table cell escaping ([69bbc11](https://github.com/sotayamashita/todo-scan/commit/69bbc1127925a9efa61aec19aa4a4cb10b38d6b7))
* **security:** comprehensive security hardening from 4-agent review ([7627ea9](https://github.com/sotayamashita/todo-scan/commit/7627ea924324efbee594c3773184c672a900d340)), closes [#125](https://github.com/sotayamashita/todo-scan/issues/125)
* **security:** escape regex metacharacters in config tags ([dd50662](https://github.com/sotayamashita/todo-scan/commit/dd50662f9aa52232ed7a6e0e1d8f300999cd3c50))
* **security:** harden output escaping and input validation ([#78](https://github.com/sotayamashita/todo-scan/issues/78)) ([a994327](https://github.com/sotayamashita/todo-scan/commit/a9943276d904f066aec46e86d8aa635a369821ec))
* **security:** strip terminal control characters from text output ([a5ddcc3](https://github.com/sotayamashita/todo-scan/commit/a5ddcc3e6d7e0b577219bc6aa2e379b767cbf795))
* **tasks:** use match instead of unwrap for clippy ([1f8c594](https://github.com/sotayamashita/todo-scan/commit/1f8c59413a87075a106da2cce03ca55331ad2dc1))
* **tasks:** wrap author mention in backticks ([b53b863](https://github.com/sotayamashita/todo-scan/commit/b53b863147bc21e4c1db07276da07db2912455d7))


### Performance Improvements

* **clean:** use LazyLock for static regex compilation ([6b96e97](https://github.com/sotayamashita/todo-scan/commit/6b96e97711b4979d26ce90cea4a33fbf68072413))
* **diff:** optimize compute_diff with git diff --name-only ([#2](https://github.com/sotayamashita/todo-scan/issues/2)) ([7d56bf4](https://github.com/sotayamashita/todo-scan/commit/7d56bf4bf16375fa649befeee72bc3462bc49eb1))
* **scanner:** add incremental scan cache ([#29](https://github.com/sotayamashita/todo-scan/issues/29)) ([a8fd5a7](https://github.com/sotayamashita/todo-scan/commit/a8fd5a70f1b7eae9d82f91e276aa125192ae0645))
* **scanner:** enable parallel directory scanning ([#3](https://github.com/sotayamashita/todo-scan/issues/3)) ([e950cf7](https://github.com/sotayamashita/todo-scan/commit/e950cf71563ea73f2431301dedb6279bcde6d61a))
