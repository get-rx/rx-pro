# Changelog

All notable changes to this project will be documented in this file.

## [0.1.26](https://github.com/get-rx/rx-pro/compare/v0.1.26...v0.1.26) (2026-02-05)

### Bug Fixes

* fix release-please not reading skip-github-release from config
* delete existing release before creating to avoid immutable release errors
* ensure all builds complete before creating release
* fix maturin Docker build by removing workspace version inheritance

## [0.1.21](https://github.com/get-rx/rx-pro/compare/v0.1.20...v0.1.21) (2026-02-05)

### Bug Fixes

* fix maturin Docker build by removing workspace version inheritance

## [0.1.19](https://github.com/get-rx/rx-pro/compare/v0.1.18...v0.1.19) (2026-02-05)


### Bug Fixes

* resolve release workflow issues ([87d348c](https://github.com/get-rx/rx-pro/commit/87d348cf85664fea85921d8959e7ca2203e30220))

## [0.1.18](https://github.com/get-rx/rx-pro/compare/v0.1.17...v0.1.18) (2026-02-04)


### Features

* add @ version syntax support to rx add command ([a1ed73a](https://github.com/get-rx/rx-pro/commit/a1ed73a3bd4c37984673f2d4940ec64d8892400f))
* add documentation, CI/CD, website, and Python bindings ([09bfa8b](https://github.com/get-rx/rx-pro/commit/09bfa8b777595e24908097660225e102632aeae3))
* add import from requirements.txt and uv.lock ([b5144e6](https://github.com/get-rx/rx-pro/commit/b5144e6d7accba7c9693e08ced0e7fd2c5ff8604))
* add Python version management, tool runner, and PEP 723 script support ([d63bae7](https://github.com/get-rx/rx-pro/commit/d63bae7c0de62ae5ed065a965690470981d73aa1))
* add self-update command with install method detection ([1159c11](https://github.com/get-rx/rx-pro/commit/1159c11a1f54811badea1bb275fec047f975e8b5))
* add specific version support to rx update command ([d695ce9](https://github.com/get-rx/rx-pro/commit/d695ce96bf7629cc9fb4caff9cacf0ba1dfe5761))
* enhance rx audit with ignore config and yanked detection ([b957769](https://github.com/get-rx/rx-pro/commit/b95776980591947b7b587cc77093609b40798716))
* implement affected detection for workspaces ([cb7861d](https://github.com/get-rx/rx-pro/commit/cb7861d9a52d93a6b6a3bd6bcdd55f87f1449156))
* implement Docker integration ([48aeb10](https://github.com/get-rx/rx-pro/commit/48aeb100193338ffd80fde2856ca6364fd345e93))
* implement dotenv support for rx run and rx shell ([827f967](https://github.com/get-rx/rx-pro/commit/827f967b4f0f408b070b326207af5a59963e857c))
* implement editable install CLI support ([ed05f7a](https://github.com/get-rx/rx-pro/commit/ed05f7ac20769c68e7d096ffaca126af5acd541e))
* implement local path dependencies ([fee393e](https://github.com/get-rx/rx-pro/commit/fee393e3a884ca72405eb1bc4ecaeae553cac01e))
* implement platform markers, dependency graph, private registry auth, and Poetry import ([3f77f7b](https://github.com/get-rx/rx-pro/commit/3f77f7b96f5d995a8cc0d1e255f6d98294439435))
* implement Polylith architecture support ([b0a50e1](https://github.com/get-rx/rx-pro/commit/b0a50e10f1bdb369513083331ee41ffcaa57d5cc))
* implement rx build command ([c584eed](https://github.com/get-rx/rx-pro/commit/c584eed661c10c09aefab1625b5b5fa88a606aa6))
* implement rx bundle command ([792863c](https://github.com/get-rx/rx-pro/commit/792863c439a1041528a9c4cff82083f68babf290))
* implement rx export command ([7bf62e5](https://github.com/get-rx/rx-pro/commit/7bf62e5a8d378f1dc202181bdeb5a3023ade1a72))
* implement rx publish command ([fbe9753](https://github.com/get-rx/rx-pro/commit/fbe97534608ed63c5c4d6eab139ad1a58b6aeb9f))
* implement rx release command ([24cfa52](https://github.com/get-rx/rx-pro/commit/24cfa52b6ccb779eefacd64be3d8490d7eacf7a1))
* implement rx remove command ([690eb9b](https://github.com/get-rx/rx-pro/commit/690eb9b7ffa14b066af94a3bed0785ed76401d6a))
* implement rx shell command ([5ee874c](https://github.com/get-rx/rx-pro/commit/5ee874ca95501f63ab0055a06042b4cda676083e))
* implement rx task command ([02d36d2](https://github.com/get-rx/rx-pro/commit/02d36d25008b212a7aae085589acab55fb98d057))
* implement script aliases in [tool.rx.scripts] ([1296132](https://github.com/get-rx/rx-pro/commit/12961329065eca353b419622092c04f5706ea5f3))
* implement WebAssembly plugin system ([de54e2a](https://github.com/get-rx/rx-pro/commit/de54e2a163076913b1cd834977af235558c0df30))
* implement workspace support for monorepo management ([239195a](https://github.com/get-rx/rx-pro/commit/239195a53b65fa4768afb8174d26f59303fea73a))


### Bug Fixes

* add PYO3_USE_ABI3_FORWARD_COMPATIBILITY for Python 3.14 ([4db3891](https://github.com/get-rx/rx-pro/commit/4db38910f3e554aa39b500c8b27e60606c25591c))
* add scroll offset for Get Started button ([01ac3c0](https://github.com/get-rx/rx-pro/commit/01ac3c09dffcaf67a5991d1f6fda6f394c62e810))
* add version requirements for crates.io and build multi-platform PyPI wheels ([e9e4de6](https://github.com/get-rx/rx-pro/commit/e9e4de6b7f67eaf29d75142f86867234c7c63acc))
* center stats section on website ([edcd1c4](https://github.com/get-rx/rx-pro/commit/edcd1c4d1b2143745b91916b8f839b2ad010d0cb))
* correct PyPI link in README footer ([6ab17bd](https://github.com/get-rx/rx-pro/commit/6ab17bd96ea2791bbfcbc7c6857882dc9ec85b6b))
* enable abi3-py38 for cross-Python version wheel compatibility ([d47992b](https://github.com/get-rx/rx-pro/commit/d47992b33f345ddacaeb92da2249019a35d7be67))
* fix CI/CD workflows and code formatting ([5a133c2](https://github.com/get-rx/rx-pro/commit/5a133c2bdd002672c22a5963c2e7894e28747210))
* remove pages enablement flag that requires extra permissions ([ab3ef90](https://github.com/get-rx/rx-pro/commit/ab3ef9004f689d5551b104bf0fdec5bdcedc0104))
* remove release-type override in workflow ([53d84ba](https://github.com/get-rx/rx-pro/commit/53d84ba4f7d09e309e0105cbf01fa89052ce15e9))
* rename PyPI package to trex-py and add curl installer ([e1a86f4](https://github.com/get-rx/rx-pro/commit/e1a86f418da475f6e903a1781d3bed2906d0d188))
* replace remaining rx_core/rx_plugin references with pro_core/pro_plugin ([11e83eb](https://github.com/get-rx/rx-pro/commit/11e83eb100045926e913ea0590c8817757ce189d))
* resolve clippy warnings and test failures in self-update ([5043cc5](https://github.com/get-rx/rx-pro/commit/5043cc5f5d9dcf2a145a12bbfda7f9dfb7919dc5))
* resolve clippy warnings and update README comparison table ([ffd2acb](https://github.com/get-rx/rx-pro/commit/ffd2acbbb8d203ead7a756b870293b5850b95cb0))
* simplify PyPI wheel builds to native platforms only ([481ca87](https://github.com/get-rx/rx-pro/commit/481ca87fe045a14042d13d2380221c82afe9aed8))
* sync pro-python version with workspace (0.1.16) ([63e4601](https://github.com/get-rx/rx-pro/commit/63e46018b5f4ff938d93cb86bf7c081cce11e6ba))
* update PyPI wheel build matrix for current runners ([f83d345](https://github.com/get-rx/rx-pro/commit/f83d345723afdf5b58cc026f8ff466679d7197d9))
* update Python bindings to match rx-core API ([7607bfb](https://github.com/get-rx/rx-pro/commit/7607bfb9fda882bbfcea29226e337af0c1b86c3f))
* upgrade PyO3 to 0.23 and fix release workflow ([1f8fc31](https://github.com/get-rx/rx-pro/commit/1f8fc3130f64580406add5f6852b8227554e0092))
* use cross for Linux ARM and musl builds ([ff58204](https://github.com/get-rx/rx-pro/commit/ff5820419db3059e4a0bf1a255e170fc4224de69))
* use simple release type for workspace compatibility ([65550e8](https://github.com/get-rx/rx-pro/commit/65550e87225982e39f299f2de90420825d94cfcc))


### Code Refactoring

* rename project from T-Rex to Pro ([3d601b2](https://github.com/get-rx/rx-pro/commit/3d601b21167f1ab30c8c871360d8fe5741bdfe58))

## [0.1.17](https://github.com/get-rx/rx-pro/compare/v0.1.16...v0.1.17) (2026-02-04)

### Bug Fixes

* remove release-type override in workflow ([53d84ba](https://github.com/get-rx/rx-pro/commit/53d84ba4f7d09e309e0105cbf01fa89052ce15e9))
* use simple release type for workspace compatibility ([65550e8](https://github.com/get-rx/rx-pro/commit/65550e87225982e39f299f2de90420825d94cfcc))

## [0.1.16](https://github.com/get-rx/rx-pro/compare/v0.1.15...v0.1.16) (2026-02-04)

### Bug Fixes

* sync pro-python version with workspace

## [0.1.15](https://github.com/get-rx/rx-pro/compare/v0.1.14...v0.1.15) (2026-02-04)

### Features

* add import from requirements.txt and uv.lock

### Bug Fixes

* fix formatting and clippy warnings in import command

## [0.1.13](https://github.com/get-rx/rx-pro/compare/v0.1.12...v0.1.13) (2026-01-27)

### Documentation

* update repo references to get-rx/rx-pro

## [0.1.12](https://github.com/get-rx/rx-pro/releases/tag/v0.1.12) (2026-01-27)

Initial public release with core features:

* Python package management with `rx add`, `rx remove`, `rx sync`
* Dependency resolution using pubgrub algorithm
* Lock file generation (`rx.lock`)
* Virtual environment management
* Import from Poetry projects
* Export to requirements.txt
* Security auditing with `rx audit`
* Docker image generation
* WebAssembly plugin system
