//! CLI 命令行接口
//!
//! 使用 clap 定义所有子命令

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// CLI 主入口
#[derive(Parser)]
#[command(name = "cc-proxy")]
#[command(version = "3.14.1")]
#[command(about = "All-in-One HTTP Proxy for Claude Code, Gemini CLI and more")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// 指定配置目录（默认 ~/.config/cc-proxy/）
    #[arg(short, long, global = true)]
    pub config_dir: Option<PathBuf>,
}

/// 子命令
#[derive(Subcommand)]
pub enum Commands {
    /// 前台启动代理服务器
    Start {
        /// 后台守护进程模式
        #[arg(short, long)]
        daemon: bool,
    },
    /// 停止后台守护进程
    Stop,
    /// 查看代理运行状态
    Status,
    /// 配置管理
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },
    /// Provider 管理
    Provider {
        #[command(subcommand)]
        action: ProviderCommands,
    },
    /// 手动触发故障转移
    Failover {
        #[command(subcommand)]
        action: FailoverCommands,
    },
}

/// 配置子命令
#[derive(Subcommand)]
pub enum ConfigCommands {
    /// 列出所有配置项
    List,
    /// 读取配置项
    Get { key: Option<String> },
    /// 设置配置项
    Set { key: String, value: String },
}

/// Provider 子命令
#[derive(Subcommand)]
pub enum ProviderCommands {
    /// 列出所有 providers
    List,
    /// 获取指定 provider 的可用模型列表
    Models { name: String },
    /// 健康检查
    Health {
        name: Option<String>,  // None means "all"
    },
    /// 测试端点延迟
    TestEndpoint {
        url: String,
    },
    /// 测试模型是否可用
    TestModel {
        /// Provider 名称
        name: String,
        /// 模型名称（可选，默认使用配置的模型）
        model: Option<String>,
    },
}

/// Failover 子命令
#[derive(Subcommand)]
pub enum FailoverCommands {
    /// 手动切换到指定 provider
    Switch { name: String },
}