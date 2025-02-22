use async_trait::async_trait;
use redgold_schema::{observability::errors::EnhanceErrorInfo, ErrorInfoContext, RgResult};
use tokio::fs::{read as tokio_read, write as tokio_write, read_to_string, write as tokio_write_string, remove_file, create_dir_all, try_exists};
use std::path::{Path, PathBuf};
use std::fs;

/// Trait for types that can be used as file paths
pub trait AsPath {
    fn to_path_from_ref(&self) -> &Path;
    fn to_display_str(&self) -> String;
}

impl<T: AsRef<Path>> AsPath for T {
    fn to_path_from_ref(&self) -> &Path {
        self.as_ref()
    }
    fn to_display_str(&self) -> String {
        self.as_ref().display().to_string()
    }
}

#[async_trait]
pub trait FileUtils {
    async fn read_bytes(&self) -> RgResult<Vec<u8>>;
    async fn write_bytes(&self, bytes: Vec<u8>) -> RgResult<()>;
    async fn read_string(&self) -> RgResult<String>;
    async fn write_string<S: AsRef<str> + Send + Sync>(&self, content: S) -> RgResult<()>;
    async fn delete_file(&self) -> RgResult<()>;
    async fn create_dirs(&self) -> RgResult<()>;
    async fn exists(&self) -> RgResult<bool>;
}

#[async_trait]
impl<T: AsPath + Send + Sync> FileUtils for T {
    async fn read_bytes(&self) -> RgResult<Vec<u8>> {
        Ok(tokio_read(self.to_path_from_ref()).await
            .error_info("tokio fs read failure")
            .with_detail("path", self.to_display_str())?)
    }

    async fn write_bytes(&self, bytes: Vec<u8>) -> RgResult<()> {
        tokio_write(self.to_path_from_ref(), bytes).await
            .error_info("tokio fs write failure")
            .with_detail("path", self.to_display_str())?;
        Ok(())
    }

    async fn read_string(&self) -> RgResult<String> {
        Ok(read_to_string(self.to_path_from_ref()).await
            .error_info("tokio fs read_to_string failure")
            .with_detail("path", self.to_display_str())?)
    }

    async fn write_string<S: AsRef<str> + Send + Sync>(&self, content: S) -> RgResult<()> {
        tokio_write_string(self.to_path_from_ref(), content.as_ref()).await
            .error_info("tokio fs write string failure")
            .with_detail("path", self.to_display_str())?;
        Ok(())
    }

    async fn delete_file(&self) -> RgResult<()> {
        let path = self.to_path_from_ref();
        if path.exists() && path.is_file() {
            remove_file(path).await
                .error_info("tokio fs delete file failure")
                .with_detail("path", self.to_display_str())?;
        }
        Ok(())
    }

    async fn create_dirs(&self) -> RgResult<()> {
        create_dir_all(self.to_path_from_ref()).await
            .error_info("tokio fs create directories failure")
            .with_detail("path", self.to_display_str())?;
        Ok(())
    }

    async fn exists(&self) -> RgResult<bool> {
        Ok(try_exists(self.to_path_from_ref()).await
            .error_info("tokio fs exists check failure")
            .with_detail("path", self.to_display_str())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn test_dir() -> PathBuf {
        let dir = PathBuf::from("target").join("file_utils_test");
        fs::create_dir_all(&dir).expect("Failed to create test directory");
        dir
    }

    #[tokio::test]
    async fn test_file_utils() -> RgResult<()> {
        let test_dir = test_dir();
        
        // Test directory creation
        let nested_dir = test_dir.join("nested").join("directories");
        assert!(!nested_dir.exists().await?);
        nested_dir.create_dirs().await?;
        assert!(nested_dir.exists().await?);
        // Test creating directory that already exists - should not fail
        nested_dir.create_dirs().await?;
        
        // Test with String path
        let file_path = test_dir.join("test.txt");
        assert!(!file_path.exists().await?);
        let path_str = file_path.to_string_lossy().to_string();
        let test_content = "Hello, World!";
        path_str.write_string(test_content).await?;
        assert!(file_path.exists().await?);
        
        // Test with PathBuf
        let read_content = file_path.read_string().await?;
        assert_eq!(read_content, test_content);
        
        // Test bytes
        let bytes = vec![1, 2, 3, 4, 5];
        let bytes_file = test_dir.join("test.bin");
        assert!(!bytes_file.exists().await?);
        bytes_file.write_bytes(bytes.clone()).await?;
        assert!(bytes_file.exists().await?);
        let read_bytes = bytes_file.read_bytes().await?;
        assert_eq!(read_bytes, bytes);

        // Test file deletion
        bytes_file.delete_file().await?;
        assert!(!bytes_file.exists().await?);
        // Test deleting non-existent file - should not fail
        bytes_file.delete_file().await?;
        
        // Test error cases
        let non_existent = test_dir.join("does_not_exist.txt");
        assert!(!non_existent.exists().await?);
        assert!(non_existent.read_string().await.is_err());
        assert!(non_existent.read_bytes().await.is_err());
        // Deleting non-existent file should not error
        assert!(non_existent.delete_file().await.is_ok());
        
        // Test deleting directory as file - should not fail
        assert!(nested_dir.delete_file().await.is_ok());
        assert!(nested_dir.exists().await?); // Directory should still exist
        
        // Cleanup
        fs::remove_dir_all(&test_dir)
            .error_info("Failed to cleanup test directory")
            .with_detail("path", test_dir.display().to_string())?;
        
        Ok(())
    }
}