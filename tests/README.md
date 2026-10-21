# Application tests

This is the dedicated directory for application test code and fixtures, including tests for application tooling. There is no Cargo workspace yet. P00 tooling tests are under `tests/tooling/`, with synthetic inputs under [fixtures/p00](fixtures/p00/README.md); ignored analysis-tool regression tests remain separate local reference checks.

Run `python3 -m unittest discover -s tests/tooling -p 'test_*.py'` from the repository root with Python 3.11+. The optional source-audit parser regression requires Node and `P00_TYPESCRIPT` pointing to TypeScript 6.0.3; it reports a skip otherwise. Set that variable for the full P00 reference check as described in the [run record](../docs/app/baseline/reference-run.md). The default suite needs only tracked inputs; no Cargo commands or audio/end-to-end checks are claimed.

Organize tests by owning crate/feature and test kind as they are implemented. Suggested areas are unit tests, integration/backend/engine tests, fixtures, performance tests and shared test support; create them only when needed. Use redacted, disposable data and keep required inputs tracked.

P01 must explicitly connect files here to the owning Cargo packages through test targets or test-module wiring, and configure CI to execute them. Tests in a virtual workspace's root are not discovered merely because this directory exists. Include tests needing private internals under the owning crate's `cfg(test)` boundary without widening production visibility. Document the actual commands in [app documentation](../docs/app/README.md) once validated.

Follow [AGENTS.md](../AGENTS.md): implement -> test -> fix/retest until passing -> two independent reviews -> fix/retest until passing -> commit/push. A fix requires rerunning the reproducing test when applicable, relevant regressions and affected checks; earlier passing results do not validate changed code.
