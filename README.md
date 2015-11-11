# RBot-parser
IRC parser with Rust using [Nom](https://crates.io/crates/nom). Extracted from RBot to its own repo to allow reuse.

## Usage
Use the parse_message function to parse an IRC protocol string.
```rust
pub fn parse_message(input: &str) -> Result<Message, ParserError>
```
Check out the tests for examples.
