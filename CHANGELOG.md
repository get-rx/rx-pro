# Changelog

All notable changes to this project will be documented in this file.

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
