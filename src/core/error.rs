#[derive(Debug, thiserror::Error)]
pub enum GxError {
    #[error("渲染错误: {0}")]
    Render(String),

    #[error("gltf 加载错误: {0}")]
    Gltf(#[from] gltf::Error),

    #[error("Surface 错误: {0}")]
    Surface(String),

    #[error("请求适配器失败")]
    RequestAdapter,

    #[error("请求设备失败: {0}")]
    RequestDevice(String),
}

pub type GxResult<T> = Result<T, GxError>;
