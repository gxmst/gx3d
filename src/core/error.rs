#[derive(Debug, thiserror::Error)]
pub enum GxError {
    #[error("渲染错误: {0}")]
    Render(String),

    #[error("资产加载失败: {path}")]
    AssetNotFound { path: String },

    #[error("物理引擎错误: {0}")]
    Physics(String),

    #[error("窗口错误: {0}")]
    Window(String),

    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("图像加载错误: {0}")]
    Image(#[from] image::ImageError),

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
