# 1.0.0 (2026-03-04)


### Bug Fixes

* **ci:** auto-pull images on up, drop macOS x86_64 targets ([ba49988](https://github.com/mattjmcnaughton/grov/commit/ba49988f9736f4d4499bb2d29ace4aba52193d0c))
* **ci:** gate native backend tests behind separate feature flag ([19bafd9](https://github.com/mattjmcnaughton/grov/commit/19bafd903dd8c0fe1ebbf1c331b3c5a0460e3a71))
* **ci:** use separate cargo target dir in native test container ([f8f08f1](https://github.com/mattjmcnaughton/grov/commit/f8f08f12b04328da45ed829b89d434872385f155))


### Features

* add demo scripts, justfile run targets, and GROV_BACKEND env var ([24d2678](https://github.com/mattjmcnaughton/grov/commit/24d26789fafdb2a6d0652314b0519fdf4b14d3e7))
* **backend:** implement native backend with Linux container test harness ([b5a855b](https://github.com/mattjmcnaughton/grov/commit/b5a855b7013958cdd25fb8e86583c07c01041ea8))
* **backend:** T-013 implement Backend trait and ServiceHandle enum ([48d0cc6](https://github.com/mattjmcnaughton/grov/commit/48d0cc6911c94513338adfba1ce30d9e6f251e40))
* **backend:** T-014 implement Docker backend with context discovery ([6a3ee6b](https://github.com/mattjmcnaughton/grov/commit/6a3ee6bafd17bef1ef79b4c6b2797b8012d7cec3))
* **backend:** T-015 implement health check with TCP connect polling ([4b877ac](https://github.com/mattjmcnaughton/grov/commit/4b877acbdcfa4765cb9b70f8bacc68b0cde40675))
* **cli:** T-016 implement CLI argument parsing with clap derive ([3dbd6e7](https://github.com/mattjmcnaughton/grov/commit/3dbd6e72036e9a701ebf7bed492945fd9472b559))
* **cli:** T-019 add exit code mapping for error differentiation ([277901e](https://github.com/mattjmcnaughton/grov/commit/277901e9c80098365b451554b7c5a0e15c57d3bf))
* **cli:** T-020 add signal handling for graceful SIGINT/Ctrl+C shutdown ([41af3fa](https://github.com/mattjmcnaughton/grov/commit/41af3fa54361f8ccf68e5b6a4c58840b0e3f56e5))
* **cli:** T-024 list available services on unknown service error ([26b70d2](https://github.com/mattjmcnaughton/grov/commit/26b70d2f5aa30dceacc7681150f041aa38ab0946))
* **errors:** T-012 implement error type hierarchy ([504c67a](https://github.com/mattjmcnaughton/grov/commit/504c67adfb880e5f7b5cb7f4280108770ea5ac0a))
* grov dev workspace services CLI ([20c2e63](https://github.com/mattjmcnaughton/grov/commit/20c2e63078ea6aea15d71b8a0a7d6c7f7387e66e))
* **orchestration:** T-006 implement grove identification via SHA-256 path hashing ([61af452](https://github.com/mattjmcnaughton/grov/commit/61af4528efe97677cd443ac14c681c409a745e56))
* **orchestration:** T-009 implement ServiceDefinition and builtin service registry ([61f2ffe](https://github.com/mattjmcnaughton/grov/commit/61f2ffe4400656372e640a8ae81d37afe121bb7a))
* **orchestration:** T-010 implement port allocation via bind-to-0 ([b86e8b8](https://github.com/mattjmcnaughton/grov/commit/b86e8b80660fb6ae555f8096abf72d120cd854c2))
* **orchestration:** T-011 implement env template rendering with minijinja ([9d86584](https://github.com/mattjmcnaughton/grov/commit/9d8658440b66ee9b794543c4da64b9b70f865a60))
* **orchestration:** T-017 implement Orchestrator with install, up, down commands ([37442da](https://github.com/mattjmcnaughton/grov/commit/37442dace1d2d2846529911dae5a558899d5e4af))
* **orchestration:** T-018 implement env and status command handlers ([d5c69ab](https://github.com/mattjmcnaughton/grov/commit/d5c69ab693281b9698b6960ff0c2c04047a3d2da))
* **orchestration:** T-023 detect and clean stale state in env and status ([5070c7c](https://github.com/mattjmcnaughton/grov/commit/5070c7cdb0636633e89bae6e8383a567df456807))
* **scaffold:** T-001 set up crate structure with module directories ([95c9b24](https://github.com/mattjmcnaughton/grov/commit/95c9b24d59eacf5f4055d6d38e9c63d0b39be1b1))
* **storage:** T-007 implement state types and JSON serialization ([5768b7e](https://github.com/mattjmcnaughton/grov/commit/5768b7e42ddd7f4f3348e8b58dfe49a132f93ced))
* **storage:** T-008 implement StateManager with atomic writes and file locking ([f2476ae](https://github.com/mattjmcnaughton/grov/commit/f2476aecdc70e34def38ed0711d82b10b66c7279))
* **tracing:** T-002 configure tracing with stderr output and verbosity flags ([08de810](https://github.com/mattjmcnaughton/grov/commit/08de810b9bef835a058ec0ad1ee41bdb504ac905))
