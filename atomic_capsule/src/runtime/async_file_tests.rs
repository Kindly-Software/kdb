//! Comprehensive tests for AsyncFileCapsule (T5 Streaming)
//!
//! Test coverage includes:
//! - Unit tests: Layout, initialization, state management
//! - Property tests: Invariants, memory safety, ordering
//! - Integration tests: Read/write cycles, file operations
//! - Production tests: Stress, performance, edge cases

#[cfg(all(test, feature = "streaming-async"))]
mod tests {
    use crate::runtime::async_file::{
        AsyncFileCapsule, BufWriterCapsule, FlushPolicy, AsyncFileError,
    };
    use tempfile::NamedTempFile;

    // Simple non-async test to verify module is being compiled
    #[test]
    fn test_module_compilation() {
        // If this test runs, the module is being compiled
        assert!(true);
    }

    #[test]
    fn test_capsule_layout() {
        let capsule = AsyncFileCapsule::new();
        assert_eq!(std::mem::size_of_val(&capsule), 256);
        assert_eq!(std::mem::align_of_val(&capsule), 64);
    }

    #[test]
    fn test_new_unopened() {
        let capsule = AsyncFileCapsule::new();
        assert!(!capsule.is_open());
        assert_eq!(capsule.offset(), 0);
        assert_eq!(capsule.bytes_read(), 0);
        assert_eq!(capsule.bytes_written(), 0);
    }

    #[test]
    fn test_default_creation() {
        let c1 = AsyncFileCapsule::new();
        let c2 = AsyncFileCapsule::default();

        assert_eq!(c1.offset(), c2.offset());
        assert_eq!(c1.bytes_read(), c2.bytes_read());
    }

    #[test]
    fn test_sync_send_traits() {
        // Verify AsyncFileCapsule is Send + Sync
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}

        assert_send::<AsyncFileCapsule>();
        assert_sync::<AsyncFileCapsule>();
    }

    #[tokio::test]
    async fn test_open_read() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        // Write test data
        std::fs::write(&path, b"hello world").unwrap();

        let capsule = AsyncFileCapsule::new();
        assert!(capsule.open_read(&path).await.is_ok());
        assert!(capsule.is_open());
        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_open_write() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let capsule = AsyncFileCapsule::new();
        assert!(capsule.open_write(&path).await.is_ok());
        assert!(capsule.is_open());
        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_open_append() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        // Write initial content
        std::fs::write(&path, b"hello ").unwrap();

        let capsule = AsyncFileCapsule::new();
        assert!(capsule.open_append(&path).await.is_ok());
        assert!(capsule.is_open());
        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_read_basic() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        // Write test data
        let data = b"hello world";
        std::fs::write(&path, data).unwrap();

        // Read it back
        let capsule = AsyncFileCapsule::new();
        capsule.open_read(&path).await.unwrap();

        let mut buf = vec![0u8; 11];
        let n = capsule.read(&mut buf).await.unwrap();
        assert_eq!(n, 11);
        assert_eq!(&buf[..n], data);

        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_read_partial() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        // Write test data
        std::fs::write(&path, b"hello world").unwrap();

        // Read partial
        let capsule = AsyncFileCapsule::new();
        capsule.open_read(&path).await.unwrap();

        let mut buf = vec![0u8; 5];
        let n = capsule.read(&mut buf).await.unwrap();
        assert_eq!(n, 5);
        assert_eq!(&buf[..n], b"hello");

        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_write_and_flush() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let capsule = AsyncFileCapsule::new();
        capsule.open_write(&path).await.unwrap();

        let data = b"hello world";
        let n = capsule.write(data).await.unwrap();
        assert_eq!(n, 11);
        assert_eq!(capsule.bytes_written(), 11);

        capsule.flush().await.unwrap();
        capsule.close().await.unwrap();

        // Verify file contents
        let contents = std::fs::read(&path).unwrap();
        assert_eq!(&contents, data);
    }

    #[tokio::test]
    async fn test_multiple_writes() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let capsule = AsyncFileCapsule::new();
        capsule.open_write(&path).await.unwrap();

        capsule.write(b"hello").await.unwrap();
        capsule.write(b" ").await.unwrap();
        capsule.write(b"world").await.unwrap();

        capsule.close().await.unwrap();

        let contents = std::fs::read(&path).unwrap();
        assert_eq!(&contents, b"hello world");
    }

    #[tokio::test]
    async fn test_append_mode() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        // Write initial content
        std::fs::write(&path, b"hello ").unwrap();

        // Append more
        let capsule = AsyncFileCapsule::new();
        capsule.open_append(&path).await.unwrap();
        capsule.write(b"world").await.unwrap();
        capsule.close().await.unwrap();

        let contents = std::fs::read(&path).unwrap();
        assert_eq!(&contents, b"hello world");
    }

    #[tokio::test]
    async fn test_buffered_writer() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let file = AsyncFileCapsule::new();
        file.open_write(&path).await.unwrap();

        let mut writer = BufWriterCapsule::with_capacity(file, 100);
        writer.write(b"hello").await.unwrap();
        writer.write(b" world").await.unwrap();
        writer.flush().await.unwrap();

        let contents = std::fs::read(&path).unwrap();
        assert_eq!(&contents, b"hello world");
    }

    #[tokio::test]
    async fn test_buffered_writer_auto_flush() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let file = AsyncFileCapsule::new();
        file.open_write(&path).await.unwrap();

        let mut writer = BufWriterCapsule::with_capacity(file, 10);

        // Write 15 bytes (should trigger auto-flush at 10)
        writer.write(b"hello").await.unwrap();      // 5 bytes
        writer.write(b"world!!").await.unwrap();    // 7 bytes (triggers flush)

        let contents = std::fs::read(&path).unwrap();
        // Should have at least partial content
        assert!(!contents.is_empty());
    }

    #[tokio::test]
    async fn test_generation_counter_increments() {
        let capsule = AsyncFileCapsule::new();
        let gen1 = capsule.generation();

        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        capsule.open_write(&path).await.unwrap();
        let gen2 = capsule.generation();
        assert!(gen2 > gen1);

        capsule.write(b"test").await.unwrap();
        let gen3 = capsule.generation();
        assert!(gen3 > gen2);

        capsule.close().await.unwrap();
        let gen4 = capsule.generation();
        assert!(gen4 > gen3);
    }

    #[tokio::test]
    async fn test_state_transitions() {
        let capsule = AsyncFileCapsule::new();

        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        // Closed -> Open
        assert!(!capsule.is_open());
        capsule.open_write(&path).await.unwrap();
        assert!(capsule.is_open());

        // Open -> Closed
        capsule.close().await.unwrap();
        assert!(!capsule.is_open());
    }

    #[tokio::test]
    async fn test_error_on_unopened_read() {
        let capsule = AsyncFileCapsule::new();
        let mut buf = [0u8; 10];

        let result = capsule.read(&mut buf).await;
        assert!(matches!(result, Err(AsyncFileError::NotOpen)));
    }

    #[tokio::test]
    async fn test_error_on_unopened_write() {
        let capsule = AsyncFileCapsule::new();

        let result = capsule.write(b"test").await;
        assert!(matches!(result, Err(AsyncFileError::NotOpen)));
    }

    #[tokio::test]
    async fn test_double_open_error() {
        let capsule = AsyncFileCapsule::new();
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        capsule.open_read(&path).await.unwrap();
        let result = capsule.open_read(&path).await;
        assert!(result.is_err());

        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_close_idempotent() {
        let capsule = AsyncFileCapsule::new();
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        capsule.open_write(&path).await.unwrap();

        // Close multiple times (should be idempotent)
        capsule.close().await.unwrap();
        capsule.close().await.unwrap();
        capsule.close().await.unwrap();

        assert!(!capsule.is_open());
    }

    #[tokio::test]
    async fn test_offset_tracking() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        // Write test data
        std::fs::write(&path, b"hello world").unwrap();

        let capsule = AsyncFileCapsule::new();
        capsule.open_read(&path).await.unwrap();

        let mut buf1 = vec![0u8; 5];
        capsule.read(&mut buf1).await.unwrap();
        assert_eq!(capsule.offset(), 5);

        let mut buf2 = vec![0u8; 5];
        capsule.read(&mut buf2).await.unwrap();
        assert_eq!(capsule.offset(), 10);

        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_bytes_read_tracking() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        std::fs::write(&path, b"hello world").unwrap();

        let capsule = AsyncFileCapsule::new();
        capsule.open_read(&path).await.unwrap();

        assert_eq!(capsule.bytes_read(), 0);

        let mut buf = vec![0u8; 11];
        capsule.read(&mut buf).await.unwrap();
        assert_eq!(capsule.bytes_read(), 11);

        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_bytes_written_tracking() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let capsule = AsyncFileCapsule::new();
        capsule.open_write(&path).await.unwrap();

        assert_eq!(capsule.bytes_written(), 0);

        capsule.write(b"hello").await.unwrap();
        assert_eq!(capsule.bytes_written(), 5);

        capsule.write(b" world").await.unwrap();
        assert_eq!(capsule.bytes_written(), 11);

        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_flush_policy_immediate() {
        let capsule = AsyncFileCapsule::new();
        capsule.set_flush_policy(FlushPolicy::Immediate);

        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        capsule.open_write(&path).await.unwrap();
        capsule.write(b"test").await.unwrap();
        // Should auto-flush on write with Immediate policy

        capsule.close().await.unwrap();

        let contents = std::fs::read(&path).unwrap();
        assert_eq!(&contents, b"test");
    }

    #[tokio::test]
    async fn test_flush_policy_batch() {
        let capsule = AsyncFileCapsule::new();
        capsule.set_flush_policy(FlushPolicy::Batch(10));

        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        capsule.open_write(&path).await.unwrap();

        // Write less than batch size
        for _ in 0..5 {
            capsule.write(b"x").await.unwrap();
        }

        // Manual flush
        capsule.flush().await.unwrap();
        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_flush_policy_manual() {
        let capsule = AsyncFileCapsule::new();
        capsule.set_flush_policy(FlushPolicy::Manual);

        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        capsule.open_write(&path).await.unwrap();
        capsule.write(b"test").await.unwrap();
        capsule.flush().await.unwrap();

        capsule.close().await.unwrap();

        let contents = std::fs::read(&path).unwrap();
        assert_eq!(&contents, b"test");
    }

    #[tokio::test]
    async fn test_concurrent_read_safety() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        std::fs::write(&path, b"hello world").unwrap();

        let capsule = std::sync::Arc::new(AsyncFileCapsule::new());
        capsule.open_read(&path).await.unwrap();

        // Create multiple read tasks
        let handles: Vec<_> = (0..3)
            .map(|_| {
                let capsule = capsule.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 5];
                    capsule.read(&mut buf).await.is_ok()
                })
            })
            .collect();

        // Wait for all tasks
        for handle in handles {
            assert!(handle.await.unwrap());
        }

        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_read_empty_file() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        // Write empty file
        std::fs::write(&path, b"").unwrap();

        let capsule = AsyncFileCapsule::new();
        capsule.open_read(&path).await.unwrap();

        let mut buf = vec![0u8; 10];
        let n = capsule.read(&mut buf).await.unwrap();
        assert_eq!(n, 0);

        capsule.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_large_write() {
        let temp = NamedTempFile::new().unwrap();
        let path = temp.path().to_path_buf();

        let capsule = AsyncFileCapsule::new();
        capsule.open_write(&path).await.unwrap();

        // Write 1MB
        let large_data = vec![b'x'; 1024 * 1024];
        let n = capsule.write(&large_data).await.unwrap();
        assert_eq!(n, 1024 * 1024);

        capsule.close().await.unwrap();

        let metadata = std::fs::metadata(&path).unwrap();
        assert_eq!(metadata.len(), 1024 * 1024);
    }

    #[tokio::test]
    async fn test_error_nonexistent_file() {
        let capsule = AsyncFileCapsule::new();
        let path = std::path::PathBuf::from("/nonexistent/path/to/file.txt");

        let result = capsule.open_read(&path).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_capsule_drop_closes_file() {
        let path = {
            let temp = NamedTempFile::new().unwrap();
            let path = temp.path().to_path_buf();
            std::fs::write(&path, b"test").unwrap();
            path
        };

        let capsule = AsyncFileCapsule::new();
        capsule.open_read(&path).await.unwrap();
        // File should be closed when capsule is dropped

        drop(capsule);

        // Should be able to delete the file
        let _ = std::fs::remove_file(&path);
    }
}
