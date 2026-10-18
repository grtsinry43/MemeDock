use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub enum ApiErrorKind {
    ParamsError,
    InvalidCredential,
    NotLogin,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    Conflict,
    TooManyRequests,
    ServerError,
    PayloadTooLarge,
    InvalidAsset,
}

impl ApiErrorKind {
    pub const fn status(self) -> u16 {
        match self {
            Self::ParamsError => 400,
            Self::InvalidCredential | Self::NotLogin => 401,
            Self::Forbidden => 403,
            Self::NotFound => 404,
            Self::MethodNotAllowed => 405,
            Self::Conflict => 409,
            Self::TooManyRequests => 429,
            Self::ServerError => 500,
            Self::PayloadTooLarge => 413,
            Self::InvalidAsset => 422,
        }
    }
    pub const fn code(self) -> u32 {
        match self {
            Self::InvalidCredential => 40101,
            _ => self.status() as u32,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::ParamsError => "PARAMS_ERROR",
            Self::InvalidCredential => "INVALID_CREDENTIAL",
            Self::NotLogin => "NOT_LOGIN",
            Self::Forbidden => "FORBIDDEN",
            Self::NotFound => "NOT_FOUND",
            Self::MethodNotAllowed => "METHOD_NOT_ALLOWED",
            Self::Conflict => "CONFLICT",
            Self::TooManyRequests => "TOO_MANY_REQUESTS",
            Self::ServerError => "SERVER_ERROR",
            Self::PayloadTooLarge => "PAYLOAD_TOO_LARGE",
            Self::InvalidAsset => "INVALID_ASSET",
        }
    }
    pub const fn message(self) -> &'static str {
        match self {
            Self::ParamsError => "请求参数无效",
            Self::InvalidCredential => "凭证无效或已撤销",
            Self::NotLogin => "请先登录",
            Self::Forbidden => "无权执行此操作",
            Self::NotFound => "内容不存在",
            Self::MethodNotAllowed => "请求方法不受支持",
            Self::Conflict => "当前状态不允许此操作",
            Self::TooManyRequests => "请求过于频繁，请稍后重试",
            Self::ServerError => "服务暂时不可用，请根据请求编号检查日志",
            Self::PayloadTooLarge => "上传内容超过大小限制",
            Self::InvalidAsset => "图片内容或哈希校验失败",
        }
    }
}
