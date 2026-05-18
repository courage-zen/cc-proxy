use indexmap::IndexMap;

use crate::app_config::AppType;
use crate::error::AppError;
use crate::prompt::Prompt;
use crate::prompt_files::prompt_file_path;
use crate::store::AppState;

pub struct PromptService;

impl PromptService {
    pub fn get_prompts(
        _state: &AppState,
        _app: AppType,
    ) -> Result<IndexMap<String, Prompt>, AppError> {
        unimplemented!()
    }

    pub fn upsert_prompt(
        _state: &AppState,
        _app: AppType,
        _id: &str,
        _prompt: Prompt,
    ) -> Result<(), AppError> {
        unimplemented!()
    }

    pub fn delete_prompt(_state: &AppState, _app: AppType, _id: &str) -> Result<(), AppError> {
        unimplemented!()
    }

    pub fn enable_prompt(_state: &AppState, _app: AppType, _id: &str) -> Result<(), AppError> {
        unimplemented!()
    }

    pub fn import_from_file(_state: &AppState, _app: AppType) -> Result<String, AppError> {
        unimplemented!()
    }

    pub fn get_current_file_content(app: AppType) -> Result<Option<String>, AppError> {
        let file_path = prompt_file_path(&app)?;
        if !file_path.exists() {
            return Ok(None);
        }
        let content =
            std::fs::read_to_string(&file_path).map_err(|e| AppError::io(&file_path, e))?;
        Ok(Some(content))
    }

    /// 首次启动时从现有提示词文件自动导入（如果存在）
    /// 返回导入的数量
    pub fn import_from_file_on_first_launch(
        _state: &AppState,
        _app: AppType,
    ) -> Result<usize, AppError> {
        unimplemented!()
    }
}
