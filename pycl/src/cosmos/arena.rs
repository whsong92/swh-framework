use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{info, warn};

/// 캐시 라인 및 SIMD 벡터화(AVX-512)를 위한 64바이트 정렬 상수
pub const MEMORY_ALIGNMENT: usize = 64;

/// COSMOS L0 Shared Physical Memory Arena
///
/// - Rust가 물리 메모리 라이프사이클을 독점 소유함.
/// - CPython(NumPy OWNDATA=False)과 Clojure(DirectByteBuffer)에게
///   가상 주소 공간이 1:1 일치하는 raw pointer(*mut f32)를 중재함.
pub struct SharedArena {
    ptr: NonNull<f32>,
    capacity: usize,
    layout: Layout,
}

// 다중 스레드(Tokio 비동기 런타임, FFI 워커) 간 안전한 참조 공유 선언
unsafe impl Send for SharedArena {}
unsafe impl Sync for SharedArena {}

impl SharedArena {
    /// 지정된 f32 요소 개수 크기로 64바이트 정렬된 물리 메모리 청크를 0으로 초기화하여 할당
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "[COSMOS/Memory] Arena capacity must be greater than zero");

        // 64-byte 캐시 라인 정렬 레이아웃 생성
        let bytes_len = capacity
            .checked_mul(std::mem::size_of::<f32>())
            .expect("[COSMOS/Memory] Capacity integer overflow");

        let layout = Layout::from_size_align(bytes_len, MEMORY_ALIGNMENT)
            .expect("[COSMOS/Memory] Failed to create 64-byte aligned layout for SharedArena");

        let raw_ptr = unsafe { alloc_zeroed(layout) as *mut f32 };
        let ptr = NonNull::new(raw_ptr)
            .expect("[COSMOS/Memory] Failed to allocate zeroed aligned memory for SharedArena");

        info!(
            "[COSMOS/Rust] L0 Shared Memory Allocated: {:p} (Capacity: {} f32 slots, Size: {} bytes, Align: {} bytes)",
            ptr.as_ptr(),
            capacity,
            layout.size(),
            layout.align()
        );

        Self {
            ptr,
            capacity,
            layout,
        }
    }

    /// L0 아레나의 총 f32 요소 용량 반환 (core.rs 호환)
    #[inline]
    pub fn size(&self) -> usize {
        self.capacity
    }

    /// L0 아레나의 총 f32 요소 용량 반환 (별칭)
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// 총 바이트 크기 반환
    #[inline]
    pub fn byte_size(&self) -> usize {
        self.capacity * std::mem::size_of::<f32>()
    }

    /// FFI(CPython C-API, JNI/C-ABI) 전달용 Raw Pointer 추출
    #[inline]
    pub fn as_mut_ptr(&self) -> *mut f32 {
        self.ptr.as_ptr()
    }

    /// 가상 메모리 시작 주소 정수형 반환
    #[inline]
    pub fn raw_address(&self) -> usize {
        self.ptr.as_ptr() as usize
    }

    /// 불변 슬라이스 뷰 반환
    #[inline]
    pub fn as_slice(&self) -> &[f32] {
        unsafe { std::slice::from_raw_parts(self.ptr.as_ptr(), self.capacity) }
    }

    /// 가변 슬라이스 뷰 반환
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.capacity) }
    }

    /// 특정 오프셋 구간의 불변 서브 슬라이스 뷰 추출
    pub fn slice_range(&self, offset: usize, length: usize) -> Option<&[f32]> {
        if offset + length <= self.capacity {
            unsafe {
                Some(std::slice::from_raw_parts(
                    self.ptr.as_ptr().add(offset),
                    length,
                ))
            }
        } else {
            warn!(
                "[COSMOS/Memory] Out of bounds slice requested: [{}..{}] (Capacity: {})",
                offset,
                offset + length,
                self.capacity
            );
            None
        }
    }

    /// 특정 오프셋 구간의 가변 서브 슬라이스 뷰 추출
    pub fn slice_range_mut(&mut self, offset: usize, length: usize) -> Option<&mut [f32]> {
        if offset + length <= self.capacity {
            unsafe {
                Some(std::slice::from_raw_parts_mut(
                    self.ptr.as_ptr().add(offset),
                    length,
                ))
            }
        } else {
            warn!(
                "[COSMOS/Memory] Out of bounds mut-slice requested: [{}..{}] (Capacity: {})",
                offset,
                offset + length,
                self.capacity
            );
            None
        }
    }

    /// 안전한 단일 슬롯 쓰기 인터페이스
    #[inline]
    pub fn write(&mut self, idx: usize, val: f32) {
        let slice = self.as_mut_slice();
        if idx < slice.len() {
            slice[idx] = val;
        } else {
            warn!(
                "[COSMOS/Memory] Out of bounds write attempted at index {} (Capacity: {})",
                idx, self.capacity
            );
        }
    }

    /// 안전한 단일 슬롯 읽기 인터페이스
    #[inline]
    pub fn read(&self, idx: usize) -> f32 {
        let slice = self.as_slice();
        if idx < slice.len() {
            slice[idx]
        } else {
            warn!(
                "[COSMOS/Memory] Out of bounds read attempted at index {} (Capacity: {})",
                idx, self.capacity
            );
            0.0
        }
    }

    /// 원자적 단일 슬롯 쓰기 (동시성 핑퐁 검증용)
    #[inline]
    pub fn write_atomic(&self, idx: usize, val: f32) {
        if idx < self.capacity {
            unsafe {
                let target = self.ptr.as_ptr().add(idx) as *const AtomicU32;
                (*target).store(val.to_bits(), Ordering::Release);
            }
        }
    }

    /// 원자적 단일 슬롯 읽기
    #[inline]
    pub fn read_atomic(&self, idx: usize) -> f32 {
        if idx < self.capacity {
            unsafe {
                let target = self.ptr.as_ptr().add(idx) as *const AtomicU32;
                f32::from_bits((*target).load(Ordering::Acquire))
            }
        } else {
            0.0
        }
    }

    /// 아레나 전체 메모리를 0.0으로 리셋
    pub fn clear(&mut self) {
        let slice = self.as_mut_slice();
        slice.fill(0.0);
    }
}

/// 아레나 해제 시 독점 소유한 물리 메모리를 명시적으로 dealloc
impl Drop for SharedArena {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr.as_ptr() as *mut u8, self.layout);
        }
        info!(
            "[COSMOS/Rust] L0 Shared Memory Deallocated: {:p} (Clean shutdown)",
            self.ptr.as_ptr()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shared_arena_64byte_alignment() {
        let arena = SharedArena::new(1024);
        assert_eq!(arena.raw_address() % MEMORY_ALIGNMENT, 0);
        assert_eq!(arena.capacity(), 1024);
    }

    #[test]
    fn test_shared_arena_range_and_atomic() {
        let mut arena = SharedArena::new(256);
        arena.write(0, 42.0);
        arena.write_atomic(1, 108.0);

        assert_eq!(arena.read(0), 42.0);
        assert_eq!(arena.read_atomic(1), 108.0);

        let sub = arena.slice_range(0, 2).unwrap();
        assert_eq!(sub, &[42.0, 108.0]);
    }
}