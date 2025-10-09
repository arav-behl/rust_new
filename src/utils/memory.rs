use memmap2::{Mmap, MmapMut, MmapOptions};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Arc;

use crate::error::Result;
use crate::EngineError;

#[derive(Debug)]
pub struct MemoryMappedRegion {
    _file: File,
    mmap: MmapMut,
    size: usize,
}

impl MemoryMappedRegion {
    pub fn create<P: AsRef<Path>>(path: P, size: usize) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .map_err(|e| EngineError::MemoryMapping(format!("Failed to create file: {}", e)))?;

        file.set_len(size as u64)
            .map_err(|e| EngineError::MemoryMapping(format!("Failed to set file length: {}", e)))?;

        let mmap = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .map_err(|e| EngineError::MemoryMapping(format!("Failed to create memory map: {}", e)))?
        };

        Ok(Self {
            _file: file,
            mmap,
            size,
        })
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| EngineError::MemoryMapping(format!("Failed to open file: {}", e)))?;

        let size = file.metadata()
            .map_err(|e| EngineError::MemoryMapping(format!("Failed to get file metadata: {}", e)))?
            .len() as usize;

        let mmap = unsafe {
            MmapOptions::new()
                .map_mut(&file)
                .map_err(|e| EngineError::MemoryMapping(format!("Failed to create memory map: {}", e)))?
        };

        Ok(Self {
            _file: file,
            mmap,
            size,
        })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.mmap[..]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.mmap[..]
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn write_at(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        if offset + data.len() > self.size {
            return Err(EngineError::MemoryMapping(
                "Write would exceed memory map bounds".to_string(),
            ));
        }

        self.mmap[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    pub fn read_at(&self, offset: usize, len: usize) -> Result<&[u8]> {
        if offset + len > self.size {
            return Err(EngineError::MemoryMapping(
                "Read would exceed memory map bounds".to_string(),
            ));
        }

        Ok(&self.mmap[offset..offset + len])
    }

    pub fn flush(&self) -> Result<()> {
        self.mmap.flush()
            .map_err(|e| EngineError::MemoryMapping(format!("Failed to flush memory map: {}", e)))
    }

    pub fn flush_async(&self) -> Result<()> {
        self.mmap.flush_async()
            .map_err(|e| EngineError::MemoryMapping(format!("Failed to async flush memory map: {}", e)))
    }
}

#[derive(Debug)]
pub struct ReadOnlyMemoryMap {
    _file: File,
    mmap: Mmap,
    size: usize,
}

impl ReadOnlyMemoryMap {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(&path)
            .map_err(|e| EngineError::MemoryMapping(format!("Failed to open file: {}", e)))?;

        let size = file.metadata()
            .map_err(|e| EngineError::MemoryMapping(format!("Failed to get file metadata: {}", e)))?
            .len() as usize;

        let mmap = unsafe {
            MmapOptions::new()
                .map(&file)
                .map_err(|e| EngineError::MemoryMapping(format!("Failed to create memory map: {}", e)))?
        };

        Ok(Self {
            _file: file,
            mmap,
            size,
        })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.mmap[..]
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn read_at(&self, offset: usize, len: usize) -> Result<&[u8]> {
        if offset + len > self.size {
            return Err(EngineError::MemoryMapping(
                "Read would exceed memory map bounds".to_string(),
            ));
        }

        Ok(&self.mmap[offset..offset + len])
    }
}

pub struct ObjectPool<T> {
    objects: parking_lot::Mutex<Vec<T>>,
    factory: Box<dyn Fn() -> T + Send + Sync>,
    max_size: usize,
}

impl<T: Default + Send + 'static> ObjectPool<T> {
    pub fn new(max_size: usize) -> Self {
        Self {
            objects: parking_lot::Mutex::new(Vec::new()),
            factory: Box::new(T::default),
            max_size,
        }
    }
}

impl<T: Send + 'static> ObjectPool<T> {
    pub fn with_factory<F>(max_size: usize, factory: F) -> Self
    where
        F: Fn() -> T + Send + Sync + 'static,
    {
        Self {
            objects: parking_lot::Mutex::new(Vec::new()),
            factory: Box::new(factory),
            max_size,
        }
    }

    pub fn get(&self) -> PooledObject<T> {
        let obj = {
            let mut objects = self.objects.lock();
            objects.pop().unwrap_or_else(|| (self.factory)())
        };

        PooledObject {
            object: Some(obj),
            pool: self,
        }
    }

    fn return_object(&self, object: T) {
        let mut objects = self.objects.lock();
        if objects.len() < self.max_size {
            objects.push(object);
        }
        // If pool is full, object is dropped
    }

    pub fn clear(&self) {
        self.objects.lock().clear();
    }

    pub fn len(&self) -> usize {
        self.objects.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.lock().is_empty()
    }
}

pub struct PooledObject<'a, T> {
    object: Option<T>,
    pool: &'a ObjectPool<T>,
}

impl<'a, T> std::ops::Deref for PooledObject<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.object.as_ref().unwrap()
    }
}

impl<'a, T> std::ops::DerefMut for PooledObject<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.object.as_mut().unwrap()
    }
}

impl<'a, T> Drop for PooledObject<'a, T> {
    fn drop(&mut self) {
        if let Some(object) = self.object.take() {
            self.pool.return_object(object);
        }
    }
}

pub struct MemoryStats {
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub available_memory_mb: u64,
    pub memory_usage_percent: f64,
}

impl MemoryStats {
    pub fn get_system_memory() -> io::Result<Self> {
        #[cfg(target_os = "macos")]
        {
            Self::get_macos_memory()
        }
        #[cfg(target_os = "linux")]
        {
            Self::get_linux_memory()
        }
        #[cfg(target_os = "windows")]
        {
            Self::get_windows_memory()
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            Ok(Self {
                total_memory_mb: 0,
                used_memory_mb: 0,
                available_memory_mb: 0,
                memory_usage_percent: 0.0,
            })
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_memory() -> io::Result<Self> {
        use std::process::Command;

        let output = Command::new("vm_stat").output()?;
        let output_str = String::from_utf8_lossy(&output.stdout);

        // Parse vm_stat output - this is a simplified version
        // In production, you'd want more robust parsing
        let page_size = 4096; // Typical page size on macOS
        let mut free_pages = 0;
        let mut inactive_pages = 0;

        for line in output_str.lines() {
            if line.contains("Pages free:") {
                if let Some(num_str) = line.split_whitespace().nth(2) {
                    free_pages = num_str.trim_end_matches('.').parse::<u64>().unwrap_or(0);
                }
            } else if line.contains("Pages inactive:") {
                if let Some(num_str) = line.split_whitespace().nth(2) {
                    inactive_pages = num_str.trim_end_matches('.').parse::<u64>().unwrap_or(0);
                }
            }
        }

        // Get total memory using sysctl (simplified)
        let total_memory_mb = 8192; // Placeholder - should use actual sysctl calls
        let available_memory_mb = (free_pages + inactive_pages) * page_size / 1024 / 1024;
        let used_memory_mb = total_memory_mb.saturating_sub(available_memory_mb);
        let memory_usage_percent = if total_memory_mb > 0 {
            (used_memory_mb as f64 / total_memory_mb as f64) * 100.0
        } else {
            0.0
        };

        Ok(Self {
            total_memory_mb,
            used_memory_mb,
            available_memory_mb,
            memory_usage_percent,
        })
    }

    #[cfg(target_os = "linux")]
    fn get_linux_memory() -> io::Result<Self> {
        let meminfo = std::fs::read_to_string("/proc/meminfo")?;

        let mut total_memory_kb = 0;
        let mut available_memory_kb = 0;

        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total_memory_kb = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
            } else if line.starts_with("MemAvailable:") {
                available_memory_kb = line.split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
            }
        }

        let total_memory_mb = total_memory_kb / 1024;
        let available_memory_mb = available_memory_kb / 1024;
        let used_memory_mb = total_memory_mb.saturating_sub(available_memory_mb);
        let memory_usage_percent = if total_memory_mb > 0 {
            (used_memory_mb as f64 / total_memory_mb as f64) * 100.0
        } else {
            0.0
        };

        Ok(Self {
            total_memory_mb,
            used_memory_mb,
            available_memory_mb,
            memory_usage_percent,
        })
    }

    #[cfg(target_os = "windows")]
    fn get_windows_memory() -> io::Result<Self> {
        // Placeholder implementation for Windows
        // In production, you'd use Windows API calls
        Ok(Self {
            total_memory_mb: 16384, // Placeholder
            used_memory_mb: 8192,   // Placeholder
            available_memory_mb: 8192, // Placeholder
            memory_usage_percent: 50.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_memory_mapped_region() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.mmap");

        let mut region = MemoryMappedRegion::create(&file_path, 1024).unwrap();

        let test_data = b"Hello, memory mapped world!";
        region.write_at(0, test_data).unwrap();

        let read_data = region.read_at(0, test_data.len()).unwrap();
        assert_eq!(read_data, test_data);

        region.flush().unwrap();
    }

    #[test]
    fn test_object_pool() {
        let pool = ObjectPool::new(10);

        {
            let obj1 = pool.get();
            let obj2 = pool.get();
            assert_eq!(pool.len(), 0); // Objects are checked out
        } // Objects should be returned here

        // After objects are returned, pool should have them
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn test_object_pool_with_factory() {
        let pool = ObjectPool::with_factory(5, || String::from("test"));

        {
            let obj = pool.get();
            assert_eq!(*obj, "test");
        }

        assert_eq!(pool.len(), 1);
    }
}