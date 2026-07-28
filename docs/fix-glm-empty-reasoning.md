# 修复：GLM-5.2 空 reasoning 字段导致 Claude Code 显示碎片化

## 现象

通过 cc-proxy 调用 GLM-5.2 后，Claude Code 渲染的回复出现异常换行：每行只有几个字，但占用很多行，内容被切成大量极短的片段。

## 根因

**位置**：`src/proxy/providers/streaming.rs:252` 的 `create_anthropic_sse_stream` 函数（OpenAI SSE → Anthropic SSE 转换器）。

GLM-5.2 上游返回的每个 SSE chunk 都带一个空字符串的 `reasoning` 字段：

```json
{"choices":[{"delta":{"reasoning":"","content":"思考 +"}}]}
```

`Delta::reasoning` 用 `#[serde(default)]` 反序列化，空字符串会变成 `Some("")`，进入 `if let Some(reasoning) = &choice.delta.reasoning` 分支后：

1. 检测到当前 block 不是 thinking → 关闭当前 text block
2. 新开一个 thinking content_block
3. 发一个空的 `thinking_delta`
4. 下一个 chunk 又带空 `reasoning` + 实际 `content` → 又关闭 thinking block → 新开 text block
5. 如此反复

结果：每个实际文本片段被塞进独立的 text block + 空 thinking block 交替，Claude Code 渲染时每个 block 都被换行处理，于是出现"每行只有几个字但占很多行"的异常显示。

这不是大模型本身的行为，是 cc-proxy 对 GLM-5.2 返回的空 `reasoning` 字段处理不当造成的协议转换 bug。

## 修复方案

### 主修复：过滤空 reasoning delta

`src/proxy/providers/streaming.rs:252`：

```rust
// 修复前
if let Some(reasoning) = &choice.delta.reasoning {

// 修复后
if let Some(reasoning) = &choice.delta.reasoning {
    if reasoning.is_empty() {
        // 跳过空 reasoning，避免反复开/关 thinking block
        // thinking block 会在 text content 或 finish_reason 到达时自动关闭
    } else {
        // 原有 block 切换 + thinking_delta 逻辑
    }
}
```

或者更简洁地：

```rust
if let Some(reasoning) = &choice.delta.reasoning {
    if !reasoning.is_empty() {
        // 原有逻辑
    }
}
```

### 不需要的修复

- **合并连续 text block**：`streaming.rs:298` 已有 `if current_non_tool_block_type != Some("text")` 保护，连续 text delta 会合并到同一个 block。日志里 text block 碎片化不是合并逻辑缺失，而是空 reasoning 反复打断 text block 导致它不停 close/reopen。修了主修复后这个现象自然消失。
- **"从非空变为空时关闭 thinking block"**：不需要。现有代码在 text content 到达或 finish_reason 设置时会自动关闭 thinking block（`streaming.rs:298-307`, `511-520`），空 reasoning 直接跳过即可。

### 错误的定位

`anthropic_to_openai_with_reasoning_content`（`src/proxy/providers/transform.rs:124`）是**请求体**转换函数（Anthropic JSON → OpenAI JSON），不处理 SSE 流。真正的 bug 在响应流转换器 `streaming.rs:252`。

## 验证

### 复现测试

模拟 GLM-5.2 的"空 reasoning + text"交替 chunk 模式：

```rust
#[tokio::test]
async fn test_streaming_empty_reasoning_does_not_fragment_text_block() {
    let input = concat!(
        "data: {\"id\":\"chatcmpl_glm\",\"model\":\"glm-5.2\",\"choices\":[{\"delta\":{\"reasoning\":\"\",\"content\":\"你好\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_glm\",\"model\":\"glm-5.2\",\"choices\":[{\"delta\":{\"reasoning\":\"\",\"content\":\"世界\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_glm\",\"model\":\"glm-5.2\",\"choices\":[{\"delta\":{\"reasoning\":\"\",\"content\":\"！\"}}]}\n\n",
        "data: {\"id\":\"chatcmpl_glm\",\"model\":\"glm-5.2\",\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":5,\"completion_tokens\":3}}\n\n",
        "data: [DONE]\n\n"
    );

    let events = collect_anthropic_events(input).await;

    // 不应出现任何 thinking block
    let thinking_starts = events.iter().filter(|e| {
        event_type(e) == Some("content_block_start")
            && e.pointer("/content_block/type").and_then(|v| v.as_str()) == Some("thinking")
    }).count();
    assert_eq!(thinking_starts, 0, "空 reasoning 不应触发 thinking block");

    // 应该只有一个 text block
    let text_starts = events.iter().filter(|e| {
        event_type(e) == Some("content_block_start")
            && e.pointer("/content_block/type").and_then(|v| v.as_str()) == Some("text")
    }).count();
    assert_eq!(text_starts, 1, "连续 text delta 应合并到同一个 block");

    // text delta 应该按顺序拼接
    let text_deltas: Vec<&str> = events.iter()
        .filter(|e| event_type(e) == Some("content_block_delta")
            && e.pointer("/delta/type").and_then(|v| v.as_str()) == Some("text_delta"))
        .filter_map(|e| e.pointer("/delta/text").and_then(|v| v.as_str()))
        .collect();
    assert_eq!(text_deltas.concat(), "你好世界！");
}
```

### 部署验证

1. 交叉编译：`CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-musl-gcc cargo build --release --target x86_64-unknown-linux-musl`
2. 部署到 82 服务器，重启 cc-proxy
3. 通过 Claude Code 调用 GLM-5.2，确认回复不再出现碎片化换行
4. 检查 `/tmp/cc-proxy.log`，确认不再出现 `thinking_delta` 空字符串 + text block 反复 open/close 的模式
