# Historical configuration

These files came from the earlier Python/HartAttack project and are kept as
reference material. The Rust library and CLI do not load them:

- `distribution.toml` describes historical instruction-family and event weights.
  Its values differ from the Rust generator's defaults.
- `chipyard.json` describes a Chipyard machine configuration.

For supported CLI configuration, use [examples/config.toml](../../examples/config.toml).
