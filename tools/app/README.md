# Application tooling

Keep maintained application build, test, benchmark, packaging and development tools here. No executable application tools exist yet; add them when the implementation needs them, document their prerequisites and usage in [application documentation](../../docs/app/README.md), and test relevant failure paths.

Put application tests and fixtures, including tests for these tools, in the dedicated [tests directory](../../tests/README.md). Tools here may run that suite but must not become a separate location for application test code.

Tools required by the application or its CI must use tracked inputs and work without the ignored rewrite inventory, archived source or analysis scripts. Existing scripts directly under `tools/` remain local rewrite-analysis references. Maintain app tools in the same task as the behavior or release process they support.
