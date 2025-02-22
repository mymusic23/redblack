use crate::structs::ErrorInfo;
use crate::{error_info, RgResult};


pub trait ToErrorInfo {
    fn to_error<T>(&self) -> RgResult<T>;
    fn to_error_info(&self) -> ErrorInfo;
}

impl ToErrorInfo for String {
    fn to_error<T>(&self) -> RgResult<T> {
        Err(error_info(self))
    }
    fn to_error_info(&self) -> ErrorInfo {
        error_info(self)
    }
}

impl ToErrorInfo for &str {
    fn to_error<T>(&self) -> RgResult<T> {
        Err::<T, ErrorInfo>(error_info(self.to_string()))

    }
    fn to_error_info(&self) -> ErrorInfo {
        error_info(self.to_string())
    }
}
