//! YAML 配置存储
//!
//! 管理 config.yaml 的读写

use crate::cli_config::AppConfig;
use anyhow::Result;
use std::path::PathBuf;

/// YAML 配置存储
pub struct YamlStore {
    dir: PathBuf,
}

impl YamlStore {
    /// 创建新的 YamlStore
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    /// 获取配置文件的完整路径
    pub fn config_path(&self) -> PathBuf {
        self.dir.join("config.yaml")
    }

    /// 加载配置，不存在时返回默认配置
    pub fn load_config(&self) -> Result<AppConfig> {
        let path = self.config_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: AppConfig = serde_yaml::from_str(&content)?;
            Ok(config)
        } else {
            // 返回默认配置
            Ok(AppConfig::default())
        }
    }

    /// 保存配置到文件
    pub fn save_config(&self, config: &AppConfig) -> Result<()> {
        // 确保目录存在
        std::fs::create_dir_all(&self.dir)?;
        let content = serde_yaml::to_string(config)?;
        std::fs::write(self.config_path(), content)?;
        Ok(())
    }

    /// 获取默认配置目录 (~/.config/cc-proxy/)
    pub fn default_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cc-proxy")
    }
}