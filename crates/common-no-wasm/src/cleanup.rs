use crate::readers_writers::FileUtils;

pub struct Cleanup(pub String);
impl Drop for Cleanup {
    fn drop(&mut self) {
        // Only spawn if we're in a Tokio context and runtime is running
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let s = self.0.clone();
            handle.spawn(async move {
                let _ = s.delete_file().await;
            });
        }
    }
}
