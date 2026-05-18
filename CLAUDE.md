# cc-proxy

## Cross-compilation: macOS ARM64 -> Linux x86_64

使用 `musl-cross` 在 macOS 上交叉编译 Linux x86_64 静态链接二进制：

```bash
# 前置：安装 musl-cross（已安装则跳过）
brew install musl-cross

# 确保 target 已添加（已添加则跳过）
rustup target add x86_64-unknown-linux-musl

# 编译
CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc \
  cargo build --release --target x86_64-unknown-linux-musl
```

产物位于 `target/x86_64-unknown-linux-musl/release/cc-proxy`，ELF 64-bit x86-64 静态链接，不依赖 glibc。

相比 `cross` 方案不需要 Docker，更轻量快速。项目有 `rusqlite`(bundled) 和 `rquickjs` 等 C 依赖，`musl-cross` 均能正确处理。
