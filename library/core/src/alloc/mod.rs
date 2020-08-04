//! Memory allocation APIs

#![stable(feature = "alloc_module", since = "1.28.0")]

mod global;
mod layout;

#[stable(feature = "global_alloc", since = "1.28.0")]
pub use self::global::GlobalAlloc;
#[stable(feature = "alloc_layout", since = "1.28.0")]
pub use self::layout::{Layout, LayoutErr};

use crate::fmt;
use crate::ptr::{self, NonNull};

/// The `AllocErr` error indicates an allocation failure
/// that may be due to resource exhaustion or to
/// something wrong when combining the given input arguments with this
/// allocator.
#[unstable(feature = "allocator_api", issue = "32838")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct AllocErr;

// (we need this for downstream impl of trait Error)
#[unstable(feature = "allocator_api", issue = "32838")]
impl fmt::Display for AllocErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("memory allocation failed")
    }
}

/// An implementation of `AllocRef` can allocate, grow, shrink, and deallocate arbitrary blocks of
/// data described via [`Layout`][].
///
/// `AllocRef` is designed to be implemented on ZSTs, references, or smart pointers because having
/// an allocator like `MyAlloc([u8; N])` cannot be moved, without updating the pointers to the
/// allocated memory.
///
/// Unlike [`GlobalAlloc`][], zero-sized allocations are allowed in `AllocRef`. If an underlying
/// allocator does not support this (like jemalloc) or return a null pointer (such as
/// `libc::malloc`), this must be caught by the implementation.
///
/// ### Currently allocated memory
///
/// Some of the methods require that a memory slice be *currently allocated* via an allocator. This
/// means that:
///
/// * the starting address for that memory slice was previously returned by [`alloc`], [`grow`], or
///   [`shrink`], and
///
/// * the memory slice has not been subsequently deallocated, where slices are either deallocated
///   directly by being passed to [`dealloc`] or were changed by being passed to [`grow`] or
///   [`shrink`] that returns `Ok`. If `grow` or `shrink` have returned `Err`, the passed pointer
///   remains valid.
///
/// [`alloc`]: AllocRef::alloc
/// [`grow`]: AllocRef::grow
/// [`shrink`]: AllocRef::shrink
/// [`dealloc`]: AllocRef::dealloc
///
/// ### Memory fitting
///
/// Some of the methods require that a layout *fit* a memory slice. What it means for a layout to
/// "fit" a memory slice means (or equivalently, for a memory slice to "fit" a layout) is that the
/// following conditions must hold:
///
/// * The slice must be allocated with the same alignment as [`layout.align()`], and
///
/// * The provided [`layout.size()`] must fall in the range `min ..= max`, where:
///   - `min` is the size of the layout most recently used to allocate the slice, and
///   - `max` is the latest actual size returned from [`alloc`], [`grow`], or [`shrink`].
///
/// [`layout.align()`]: Layout::align
/// [`layout.size()`]: Layout::size
///
/// # Safety
///
/// * Memory slices returned from an allocator must point to valid memory and retain their validity
///   until the instance and all of its clones are dropped,
///
/// * cloning or moving the allocator must not invalidate memory slices returned from this
///   allocator. A cloned allocator must behave like the same allocator, and
///
/// * any pointer to a memory slice which is [*currently allocated*] may be passed to any other
///   method of the allocator.
///
/// [*currently allocated*]: #currently-allocated-memory
#[unstable(feature = "allocator_api", issue = "32838")]
pub unsafe trait AllocRef {
    /// Attempts to allocate a slice of memory.
    ///
    /// On success, returns a [`NonNull<[u8]>`] meeting the size and alignment guarantees of `layout`.
    ///
    /// The returned slice may have a larger size than specified by `layout.size()`, and may or may
    /// not have its contents initialized.
    ///
    /// [`NonNull<[u8]>`]: NonNull
    ///
    /// # Errors
    ///
    /// Returning `Err` indicates that either memory is exhausted or `layout` does not meet
    /// allocator's size or alignment constraints.
    ///
    /// Implementations are encouraged to return `Err` on memory exhaustion rather than panicking or
    /// aborting, but this is not a strict requirement. (Specifically: it is *legal* to implement
    /// this trait atop an underlying native allocation library that aborts on memory exhaustion.)
    ///
    /// Clients wishing to abort computation in response to an allocation error are encouraged to
    /// call the [`handle_alloc_error`] function, rather than directly invoking `panic!` or similar.
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    fn alloc(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocErr>;

    /// Behaves like `alloc`, but also ensures that the returned memory is zero-initialized.
    ///
    /// # Errors
    ///
    /// Returning `Err` indicates that either memory is exhausted or `layout` does not meet
    /// allocator's size or alignment constraints.
    ///
    /// Implementations are encouraged to return `Err` on memory exhaustion rather than panicking or
    /// aborting, but this is not a strict requirement. (Specifically: it is *legal* to implement
    /// this trait atop an underlying native allocation library that aborts on memory exhaustion.)
    ///
    /// Clients wishing to abort computation in response to an allocation error are encouraged to
    /// call the [`handle_alloc_error`] function, rather than directly invoking `panic!` or similar.
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    fn alloc_zeroed(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocErr> {
        let ptr = self.alloc(layout)?;
        // SAFETY: `alloc` returns a valid memory slice
        unsafe { ptr.as_non_null_ptr().as_ptr().write_bytes(0, ptr.len()) }
        Ok(ptr)
    }

    /// Deallocates the memory slice referenced by `ptr`.
    ///
    /// # Safety
    ///
    /// * `ptr` must denote a slice of memory [*currently allocated*] via this allocator, and
    /// * `layout` must [*fit*] that slice of memory.
    ///
    /// [*currently allocated*]: #currently-allocated-memory
    /// [*fit*]: #memory-fitting
    unsafe fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout);

    /// Attempts to extend the memory slice.
    ///
    /// Returns a new [`NonNull<[u8]>`] containing a pointer and the actual size of the allocated
    /// memory. The pointer is suitable for holding data described by a new layout with `layout`’s
    /// alignment and a size given by `new_size`. To accomplish this, the allocator may extend the
    /// allocation referenced by `ptr` to fit the new layout.
    ///
    /// If this method returns `Err`, then ownership of the memory block has not been transferred to
    /// this allocator, and the contents of the memory block are unaltered.
    ///
    /// [`NonNull<[u8]>`]: NonNull
    ///
    /// # Safety
    ///
    /// * `ptr` must denote a block of memory [*currently allocated*] via this allocator,
    /// * `layout` must [*fit*] that block of memory (The `new_size` argument need not fit it.),
    /// * `new_size` must be greater than or equal to `layout.size()`, and
    /// * `new_size`, when rounded up to the nearest multiple of `layout.align()`, must not overflow
    ///   (i.e., the rounded value must be less than or equal to `usize::MAX`).
    // Note: We can't require that `new_size` is strictly greater than `layout.size()` because of ZSTs.
    // alternative: `new_size` must be strictly greater than `layout.size()` or both are zero
    ///
    /// [*currently allocated*]: #currently-allocated-memory
    /// [*fit*]: #memory-fitting
    ///
    /// # Errors
    ///
    /// Returns `Err` if the new layout does not meet the allocator's size and alignment
    /// constraints of the allocator, or if growing otherwise fails.
    ///
    /// Implementations are encouraged to return `Err` on memory exhaustion rather than panicking or
    /// aborting, but this is not a strict requirement. (Specifically: it is *legal* to implement
    /// this trait atop an underlying native allocation library that aborts on memory exhaustion.)
    ///
    /// Clients wishing to abort computation in response to an allocation error are encouraged to
    /// call the [`handle_alloc_error`] function, rather than directly invoking `panic!` or similar.
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    unsafe fn grow(
        &mut self,
        ptr: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        let size = layout.size();
        debug_assert!(
            new_size >= size,
            "`new_size` must be greater than or equal to `layout.size()`"
        );

        if size == new_size {
            return Ok(NonNull::slice_from_raw_parts(ptr, size));
        }

        let new_layout =
            // SAFETY: the caller must ensure that the `new_size` does not overflow.
            // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid for a Layout.
            // The caller must ensure that `new_size` is greater than or equal to zero. If it's equal
            // to zero, it's catched beforehand.
            unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new_ptr = self.alloc(new_layout)?;

        // SAFETY: because `new_size` must be greater than or equal to `size`, both the old and new
        // memory allocation are valid for reads and writes for `size` bytes. Also, because the old
        // allocation wasn't yet deallocated, it cannot overlap `new_ptr`. Thus, the call to
        // `copy_nonoverlapping` is safe.
        // The safety contract for `dealloc` must be upheld by the caller.
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_non_null_ptr().as_ptr(), size);
            self.dealloc(ptr, layout);
            Ok(new_ptr)
        }
    }

    /// Behaves like `grow`, but also ensures that the new contents are set to zero before being
    /// returned.
    ///
    /// The memory block will contain the following contents after a successful call to
    /// `grow_zeroed`:
    ///   * Bytes `0..layout.size()` are preserved from the original allocation.
    ///   * Bytes `layout.size()..old_size` will either be preserved or zeroed,
    ///     depending on the allocator implementation. `old_size` refers to the size of
    ///     the `MemoryBlock` prior to the `grow_zeroed` call, which may be larger than the size
    ///     that was originally requested when it was allocated.
    ///   * Bytes `old_size..new_size` are zeroed. `new_size` refers to
    ///     the size of the `MemoryBlock` returned by the `grow` call.
    ///
    /// # Safety
    ///
    /// * `ptr` must denote a block of memory [*currently allocated*] via this allocator,
    /// * `layout` must [*fit*] that block of memory (The `new_size` argument need not fit it.),
    /// * `new_size` must be greater than or equal to `layout.size()`, and
    /// * `new_size`, when rounded up to the nearest multiple of `layout.align()`, must not overflow
    ///   (i.e., the rounded value must be less than or equal to `usize::MAX`).
    // Note: We can't require that `new_size` is strictly greater than `layout.size()` because of ZSTs.
    // alternative: `new_size` must be strictly greater than `layout.size()` or both are zero
    ///
    /// [*currently allocated*]: #currently-allocated-memory
    /// [*fit*]: #memory-fitting
    ///
    /// # Errors
    ///
    /// Returns `Err` if the new layout does not meet the allocator's size and alignment
    /// constraints of the allocator, or if growing otherwise fails.
    ///
    /// Implementations are encouraged to return `Err` on memory exhaustion rather than panicking or
    /// aborting, but this is not a strict requirement. (Specifically: it is *legal* to implement
    /// this trait atop an underlying native allocation library that aborts on memory exhaustion.)
    ///
    /// Clients wishing to abort computation in response to an allocation error are encouraged to
    /// call the [`handle_alloc_error`] function, rather than directly invoking `panic!` or similar.
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    unsafe fn grow_zeroed(
        &mut self,
        ptr: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        let size = layout.size();
        debug_assert!(
            new_size >= size,
            "`new_size` must be greater than or equal to `layout.size()`"
        );

        if size == new_size {
            return Ok(NonNull::slice_from_raw_parts(ptr, size));
        }

        let new_layout =
            // SAFETY: the caller must ensure that the `new_size` does not overflow.
            // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid for a Layout.
            // The caller must ensure that `new_size` is greater than or equal to zero. If it's equal
            // to zero, it's caught beforehand.
            unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new_ptr = self.alloc_zeroed(new_layout)?;

        // SAFETY: because `new_size` must be greater than or equal to `size`, both the old and new
        // memory allocation are valid for reads and writes for `size` bytes. Also, because the old
        // allocation wasn't yet deallocated, it cannot overlap `new_ptr`. Thus, the call to
        // `copy_nonoverlapping` is safe.
        // The safety contract for `dealloc` must be upheld by the caller.
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_non_null_ptr().as_ptr(), size);
            self.dealloc(ptr, layout);
            Ok(new_ptr)
        }
    }

    /// Attempts to shrink the memory slice.
    ///
    /// Returns a new [`NonNull<[u8]>`] containing a pointer and the actual size of the allocated
    /// memory. The pointer is suitable for holding data described by a new layout with `layout`’s
    /// alignment and a size given by `new_size`. To accomplish this, the allocator may shrink the
    /// allocation referenced by `ptr` to fit the new layout.
    ///
    /// If this returns `Ok`, then ownership of the memory slice referenced by `ptr` has been
    /// transferred to this allocator. The memory may or may not have been freed, and should be
    /// considered unusable unless it was transferred back to the caller again via the
    /// return value of this method.
    ///
    /// If this method returns `Err`, then ownership of the memory slice has not been transferred to
    /// this allocator, and the contents of the memory slice are unaltered.
    ///
    /// [`NonNull<[u8]>`]: NonNull
    ///
    /// # Safety
    ///
    /// * `ptr` must denote a slice of memory [*currently allocated*] via this allocator,
    /// * `layout` must [*fit*] that slice of memory (The `new_size` argument need not fit it.), and
    /// * `new_size` must be smaller than or equal to `layout.size()`.
    // Note: We can't require that `new_size` is strictly smaller than `layout.size()` because of ZSTs.
    // alternative: `new_size` must be smaller than `layout.size()` or both are zero
    ///
    /// [*currently allocated*]: #currently-allocated-memory
    /// [*fit*]: #memory-fitting
    ///
    /// # Errors
    ///
    /// Returns `Err` if the new layout does not meet the allocator's size and alignment
    /// constraints of the allocator, or if shrinking otherwise fails.
    ///
    /// Implementations are encouraged to return `Err` on memory exhaustion rather than panicking or
    /// aborting, but this is not a strict requirement. (Specifically: it is *legal* to implement
    /// this trait atop an underlying native allocation library that aborts on memory exhaustion.)
    ///
    /// Clients wishing to abort computation in response to an allocation error are encouraged to
    /// call the [`handle_alloc_error`] function, rather than directly invoking `panic!` or similar.
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    unsafe fn shrink(
        &mut self,
        ptr: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        let size = layout.size();
        debug_assert!(
            new_size <= size,
            "`new_size` must be smaller than or equal to `layout.size()`"
        );

        if size == new_size {
            return Ok(NonNull::slice_from_raw_parts(ptr, size));
        }

        let new_layout =
        // SAFETY: the caller must ensure that the `new_size` does not overflow.
        // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid for a Layout.
        // The caller must ensure that `new_size` is greater than zero.
            unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new_ptr = self.alloc(new_layout)?;

        // SAFETY: because `new_size` must be lower than or equal to `size`, both the old and new
        // memory allocation are valid for reads and writes for `new_size` bytes. Also, because the
        // old allocation wasn't yet deallocated, it cannot overlap `new_ptr`. Thus, the call to
        // `copy_nonoverlapping` is safe.
        // The safety contract for `dealloc` must be upheld by the caller.
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_non_null_ptr().as_ptr(), size);
            self.dealloc(ptr, layout);
            Ok(new_ptr)
        }
    }

    /// Creates a "by reference" adaptor for this instance of `AllocRef`.
    ///
    /// The returned adaptor also implements `AllocRef` and will simply borrow this.
    #[inline(always)]
    fn by_ref(&mut self) -> &mut Self {
        self
    }
}

#[unstable(feature = "allocator_api", issue = "32838")]
unsafe impl<A> AllocRef for &mut A
where
    A: AllocRef + ?Sized,
{
    #[inline]
    fn alloc(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocErr> {
        (**self).alloc(layout)
    }

    #[inline]
    fn alloc_zeroed(&mut self, layout: Layout) -> Result<NonNull<[u8]>, AllocErr> {
        (**self).alloc_zeroed(layout)
    }

    #[inline]
    unsafe fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: the safety contract must be upheld by the caller
        unsafe { (**self).dealloc(ptr, layout) }
    }

    #[inline]
    unsafe fn grow(
        &mut self,
        ptr: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        // SAFETY: the safety contract must be upheld by the caller
        unsafe { (**self).grow(ptr, layout, new_size) }
    }

    #[inline]
    unsafe fn grow_zeroed(
        &mut self,
        ptr: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        // SAFETY: the safety contract must be upheld by the caller
        unsafe { (**self).grow_zeroed(ptr, layout, new_size) }
    }

    #[inline]
    unsafe fn shrink(
        &mut self,
        ptr: NonNull<u8>,
        layout: Layout,
        new_size: usize,
    ) -> Result<NonNull<[u8]>, AllocErr> {
        // SAFETY: the safety contract must be upheld by the caller
        unsafe { (**self).shrink(ptr, layout, new_size) }
    }
}
