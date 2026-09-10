# Tests

- Unit tests live next to the code (`cargo test`).
- Tests marked `#[ignore]` talk to the network or the local machine (real panel
  API, Microsoft device-code endpoint, the `java` on the PATH). Run them on
  demand:

```bash
cargo test -- --ignored
```
