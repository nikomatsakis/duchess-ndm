# Implementing Java interfaces

It is possible to implement Java interfaces from Rust using the `#[duchess::impl_java_interface]` macro.
This support also requires using the duchess `build.rs` plugin for your Rust project.

```rust
#[duchess::impl_java_interface]
impl 