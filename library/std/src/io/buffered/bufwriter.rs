use crate::error;
use crate::fmt;
use crate::io::{
    self, Error, ErrorKind, IntoInnerError, IoSlice, Seek, SeekFrom, Write, DEFAULT_BUF_SIZE,
};
use crate::mem;
use crate::ptr;

/// Wraps a writer and buffers its output.
///
/// It can be excessively inefficient to work directly with something that
/// implements [`Write`]. For example, every call to
/// [`write`][`TcpStream::write`] on [`TcpStream`] results in a system call. A
/// `BufWriter<W>` keeps an in-memory buffer of data and writes it to an underlying
/// writer in large, infrequent batches.
///
/// `BufWriter<W>` can improve the speed of programs that make *small* and
/// *repeated* write calls to the same file or network socket. It does not
/// help when writing very large amounts at once, or writing just one or a few
/// times. It also provides no advantage when writing to a destination that is
/// in memory, like a [`Vec`]`<u8>`.
///
/// It is critical to call [`flush`] before `BufWriter<W>` is dropped. Though
/// dropping will attempt to flush the contents of the buffer, any errors
/// that happen in the process of dropping will be ignored. Calling [`flush`]
/// ensures that the buffer is empty and thus dropping will not even attempt
/// file operations.
///
/// # Examples
///
/// Let's write the numbers one through ten to a [`TcpStream`]:
///
/// ```no_run
/// use std::io::prelude::*;
/// use std::net::TcpStream;
///
/// let mut stream = TcpStream::connect("127.0.0.1:34254").unwrap();
///
/// for i in 0..10 {
///     stream.write(&[i+1]).unwrap();
/// }
/// ```
///
/// Because we're not buffering, we write each one in turn, incurring the
/// overhead of a system call per byte written. We can fix this with a
/// `BufWriter<W>`:
///
/// ```no_run
/// use std::io::prelude::*;
/// use std::io::BufWriter;
/// use std::net::TcpStream;
///
/// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
///
/// for i in 0..10 {
///     stream.write(&[i+1]).unwrap();
/// }
/// stream.flush().unwrap();
/// ```
///
/// By wrapping the stream with a `BufWriter<W>`, these ten writes are all grouped
/// together by the buffer and will all be written out in one system call when
/// the `stream` is flushed.
///
// HACK(#78696): can't use `crate` for associated items
/// [`TcpStream::write`]: super::super::super::net::TcpStream::write
/// [`TcpStream`]: crate::net::TcpStream
/// [`flush`]: BufWriter::flush
#[stable(feature = "rust1", since = "1.0.0")]
pub struct BufWriter<W: Write> {
    inner: Option<W>,
    // The buffer. Avoid using this like a normal `Vec` in common code paths.
    // That is, don't use `buf.push`, `buf.extend_from_slice`, or any other
    // methods that require bounds checking or the like. This makes an enormous
    // difference to performance (we may want to stop using a `Vec` entirely).
    buf: Vec<u8>,
    // #30888: If the inner writer panics in a call to write, we don't want to
    // write the buffered data a second time in BufWriter's destructor. This
    // flag tells the Drop impl if it should skip the flush.
    panicked: bool,
}

impl<W: Write> BufWriter<W> {
    /// Creates a new `BufWriter<W>` with a default buffer capacity. The default is currently 8 KB,
    /// but may change in the future.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn new(inner: W) -> BufWriter<W> {
        BufWriter::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    /// Creates a new `BufWriter<W>` with the specified buffer capacity.
    ///
    /// # Examples
    ///
    /// Creating a buffer with a buffer of a hundred bytes.
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:34254").unwrap();
    /// let mut buffer = BufWriter::with_capacity(100, stream);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn with_capacity(capacity: usize, inner: W) -> BufWriter<W> {
        BufWriter { inner: Some(inner), buf: Vec::with_capacity(capacity), panicked: false }
    }

    /// Send data in our local buffer into the inner writer, looping as
    /// necessary until either it's all been sent or an error occurs.
    ///
    /// Because all the data in the buffer has been reported to our owner as
    /// "successfully written" (by returning nonzero success values from
    /// `write`), any 0-length writes from `inner` must be reported as i/o
    /// errors from this method.
    pub(in crate::io) fn flush_buf(&mut self) -> io::Result<()> {
        /// Helper struct to ensure the buffer is updated after all the writes
        /// are complete. It tracks the number of written bytes and drains them
        /// all from the front of the buffer when dropped.
        struct BufGuard<'a> {
            buffer: &'a mut Vec<u8>,
            written: usize,
        }

        impl<'a> BufGuard<'a> {
            fn new(buffer: &'a mut Vec<u8>) -> Self {
                Self { buffer, written: 0 }
            }

            /// The unwritten part of the buffer
            fn remaining(&self) -> &[u8] {
                &self.buffer[self.written..]
            }

            /// Flag some bytes as removed from the front of the buffer
            fn consume(&mut self, amt: usize) {
                self.written += amt;
            }

            /// true if all of the bytes have been written
            fn done(&self) -> bool {
                self.written >= self.buffer.len()
            }
        }

        impl Drop for BufGuard<'_> {
            fn drop(&mut self) {
                if self.written > 0 {
                    if self.done() {
                        self.buffer.clear();
                    } else {
                        self.buffer.drain(..self.written);
                    }
                }
            }
        }

        let mut guard = BufGuard::new(&mut self.buf);
        let inner = self.inner.as_mut().unwrap();
        while !guard.done() {
            self.panicked = true;
            let r = inner.write(guard.remaining());
            self.panicked = false;

            match r {
                Ok(0) => {
                    return Err(Error::new_const(
                        ErrorKind::WriteZero,
                        &"failed to write the buffered data",
                    ));
                }
                Ok(n) => guard.consume(n),
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Buffer some data without flushing it, regardless of the size of the
    /// data. Writes as much as possible without exceeding capacity. Returns
    /// the number of bytes written.
    pub(super) fn write_to_buf(&mut self, buf: &[u8]) -> usize {
        let available = self.buf.capacity() - self.buf.len();
        let amt_to_buffer = available.min(buf.len());

        // SAFETY: `amt_to_buffer` is <= buffer's spare capacity by construction.
        unsafe {
            self.write_to_buffer_unchecked(&buf[..amt_to_buffer]);
        }

        amt_to_buffer
    }

    /// Gets a reference to the underlying writer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // we can use reference just like buffer
    /// let reference = buffer.get_ref();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_ref(&self) -> &W {
        self.inner.as_ref().unwrap()
    }

    /// Gets a mutable reference to the underlying writer.
    ///
    /// It is inadvisable to directly write to the underlying writer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // we can use reference just like buffer
    /// let reference = buffer.get_mut();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.as_mut().unwrap()
    }

    /// Returns a reference to the internally buffered data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let buf_writer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // See how many bytes are currently buffered
    /// let bytes_buffered = buf_writer.buffer().len();
    /// ```
    #[stable(feature = "bufreader_buffer", since = "1.37.0")]
    pub fn buffer(&self) -> &[u8] {
        &self.buf
    }

    /// Returns a mutable reference to the internal buffer.
    ///
    /// This can be used to write data directly into the buffer without triggering writers
    /// to the underlying writer.
    ///
    /// That the buffer is a `Vec` is an implementation detail.
    /// Callers should not modify the capacity as there currently is no public API to do so
    /// and thus any capacity changes would be unexpected by the user.
    pub(in crate::io) fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }

    /// Returns the number of bytes the internal buffer can hold without flushing.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let buf_writer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // Check the capacity of the inner buffer
    /// let capacity = buf_writer.capacity();
    /// // Calculate how many bytes can be written without flushing
    /// let without_flush = capacity - buf_writer.buffer().len();
    /// ```
    #[stable(feature = "buffered_io_capacity", since = "1.46.0")]
    pub fn capacity(&self) -> usize {
        self.buf.capacity()
    }

    /// Unwraps this `BufWriter<W>`, returning the underlying writer.
    ///
    /// The buffer is written out before returning the writer.
    ///
    /// # Errors
    ///
    /// An [`Err`] will be returned if an error occurs while flushing the buffer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // unwrap the TcpStream and flush the buffer
    /// let stream = buffer.into_inner().unwrap();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn into_inner(mut self) -> Result<W, IntoInnerError<BufWriter<W>>> {
        match self.flush_buf() {
            Err(e) => Err(IntoInnerError::new(self, e)),
            Ok(()) => Ok(self.inner.take().unwrap()),
        }
    }

    /// Disassembles this `BufWriter<W>`, returning the underlying writer, and any buffered but
    /// unwritten data.
    ///
    /// If the underlying writer panicked, it is not known what portion of the data was written.
    /// In this case, we return `WriterPanicked` for the buffered data (from which the buffer
    /// contents can still be recovered).
    ///
    /// `into_raw_parts` makes no attempt to flush data and cannot fail.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(bufwriter_into_raw_parts)]
    /// use std::io::{BufWriter, Write};
    ///
    /// let mut buffer = [0u8; 10];
    /// let mut stream = BufWriter::new(buffer.as_mut());
    /// write!(stream, "too much data").unwrap();
    /// stream.flush().expect_err("it doesn't fit");
    /// let (recovered_writer, buffered_data) = stream.into_raw_parts();
    /// assert_eq!(recovered_writer.len(), 0);
    /// assert_eq!(&buffered_data.unwrap(), b"ata");
    /// ```
    #[unstable(feature = "bufwriter_into_raw_parts", issue = "80690")]
    pub fn into_raw_parts(mut self) -> (W, Result<Vec<u8>, WriterPanicked>) {
        let buf = mem::take(&mut self.buf);
        let buf = if !self.panicked { Ok(buf) } else { Err(WriterPanicked { buf }) };
        (self.inner.take().unwrap(), buf)
    }

    // Ensure this function does not get inlined into `write`, so that it
    // remains inlineable and its common path remains as short as possible.
    // If this function ends up being called frequently relative to `write`,
    // it's likely a sign that the client is using an improperly sized buffer
    // or their write patterns are somewhat pathological.
    #[inline(never)]
    fn write_cold(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.buf.len() + buf.len() > self.buf.capacity() {
            self.flush_buf()?;
        }

        // Why not len > capacity? To avoid a needless trip through the buffer when the input
        // exactly fills it. We'd just need to flush it to the underlying writer anyway.
        if buf.len() >= self.buf.capacity() {
            self.panicked = true;
            let r = self.get_mut().write(buf);
            self.panicked = false;
            r
        } else {
            // Write to the buffer. In this case, we write to the buffer even if it fills it
            // exactly. Doing otherwise would mean flushing the buffer, then writing this
            // input to the inner writer, which in many cases would be a worse strategy.

            // SAFETY: We just called `self.flush_buf()`, so `self.buf.len()` is 0, and
            // we entered this else block because `buf.len() < self.buf.capacity()`.
            // Therefore, `self.buf.len() + buf.len() <= self.buf.capacity()`.
            unsafe {
                self.write_to_buffer_unchecked(buf);
            }

            Ok(buf.len())
        }
    }

    // Ensure this function does not get inlined into `write_all`, so that it
    // remains inlineable and its common path remains as short as possible.
    // If this function ends up being called frequently relative to `write_all`,
    // it's likely a sign that the client is using an improperly sized buffer
    // or their write patterns are somewhat pathological.
    #[inline(never)]
    fn write_all_cold(&mut self, buf: &[u8]) -> io::Result<()> {
        // Normally, `write_all` just calls `write` in a loop. We can do better
        // by calling `self.get_mut().write_all()` directly, which avoids
        // round trips through the buffer in the event of a series of partial
        // writes in some circumstances.
        if self.buf.len() + buf.len() > self.buf.capacity() {
            self.flush_buf()?;
        }

        // Why not len > capacity? To avoid a needless trip through the buffer when the input
        // exactly fills it. We'd just need to flush it to the underlying writer anyway.
        if buf.len() >= self.buf.capacity() {
            self.panicked = true;
            let r = self.get_mut().write_all(buf);
            self.panicked = false;
            r
        } else {
            // Write to the buffer. In this case, we write to the buffer even if it fills it
            // exactly. Doing otherwise would mean flushing the buffer, then writing this
            // input to the inner writer, which in many cases would be a worse strategy.

            // SAFETY: We just called `self.flush_buf()`, so `self.buf.len()` is 0, and
            // we entered this else block because `buf.len() < self.buf.capacity()`.
            // Therefore, `self.buf.len() + buf.len() <= self.buf.capacity()`.
            unsafe {
                self.write_to_buffer_unchecked(buf);
            }

            Ok(())
        }
    }

    // SAFETY: Requires `self.buf.len() + buf.len() <= self.buf.capacity()`,
    // i.e., that input buffer length is less than or equal to spare capacity.
    #[inline(always)]
    unsafe fn write_to_buffer_unchecked(&mut self, buf: &[u8]) {
        debug_assert!(self.buf.len() + buf.len() <= self.buf.capacity());
        let old_len = self.buf.len();
        let buf_len = buf.len();
        let src = buf.as_ptr();
        let dst = self.buf.as_mut_ptr().add(old_len);
        ptr::copy_nonoverlapping(src, dst, buf_len);
        self.buf.set_len(old_len + buf_len);
    }
}

#[unstable(feature = "bufwriter_into_raw_parts", issue = "80690")]
/// Error returned for the buffered data from `BufWriter::into_raw_parts`, when the underlying
/// writer has previously panicked.  Contains the (possibly partly written) buffered data.
///
/// # Example
///
/// ```
/// #![feature(bufwriter_into_raw_parts)]
/// use std::io::{self, BufWriter, Write};
/// use std::panic::{catch_unwind, AssertUnwindSafe};
///
/// struct PanickingWriter;
/// impl Write for PanickingWriter {
///   fn write(&mut self, buf: &[u8]) -> io::Result<usize> { panic!() }
///   fn flush(&mut self) -> io::Result<()> { panic!() }
/// }
///
/// let mut stream = BufWriter::new(PanickingWriter);
/// write!(stream, "some data").unwrap();
/// let result = catch_unwind(AssertUnwindSafe(|| {
///     stream.flush().unwrap()
/// }));
/// assert!(result.is_err());
/// let (recovered_writer, buffered_data) = stream.into_raw_parts();
/// assert!(matches!(recovered_writer, PanickingWriter));
/// assert_eq!(buffered_data.unwrap_err().into_inner(), b"some data");
/// ```
pub struct WriterPanicked {
    buf: Vec<u8>,
}

impl WriterPanicked {
    /// Returns the perhaps-unwritten data.  Some of this data may have been written by the
    /// panicking call(s) to the underlying writer, so simply writing it again is not a good idea.
    #[unstable(feature = "bufwriter_into_raw_parts", issue = "80690")]
    pub fn into_inner(self) -> Vec<u8> {
        self.buf
    }

    const DESCRIPTION: &'static str =
        "BufWriter inner writer panicked, what data remains unwritten is not known";
}

#[unstable(feature = "bufwriter_into_raw_parts", issue = "80690")]
impl error::Error for WriterPanicked {
    #[allow(deprecated, deprecated_in_future)]
    fn description(&self) -> &str {
        Self::DESCRIPTION
    }
}

#[unstable(feature = "bufwriter_into_raw_parts", issue = "80690")]
impl fmt::Display for WriterPanicked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", Self::DESCRIPTION)
    }
}

#[unstable(feature = "bufwriter_into_raw_parts", issue = "80690")]
impl fmt::Debug for WriterPanicked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriterPanicked")
            .field("buffer", &format_args!("{}/{}", self.buf.len(), self.buf.capacity()))
            .finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: Write> Write for BufWriter<W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Use < instead of <= to avoid a needless trip through the buffer in some cases.
        // See `write_cold` for details.
        if self.buf.len() + buf.len() < self.buf.capacity() {
            // SAFETY: safe by above conditional.
            unsafe {
                self.write_to_buffer_unchecked(buf);
            }

            Ok(buf.len())
        } else {
            self.write_cold(buf)
        }
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        // Use < instead of <= to avoid a needless trip through the buffer in some cases.
        // See `write_all_cold` for details.
        if self.buf.len() + buf.len() < self.buf.capacity() {
            // SAFETY: safe by above conditional.
            unsafe {
                self.write_to_buffer_unchecked(buf);
            }

            Ok(())
        } else {
            self.write_all_cold(buf)
        }
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        // FIXME: Consider applying `#[inline]` / `#[inline(never)]` optimizations already applied
        // to `write` and `write_all`. The performance benefits can be significant. See #79930.
        if self.get_ref().is_write_vectored() {
            let total_len = bufs.iter().map(|b| b.len()).sum::<usize>();
            if self.buf.len() + total_len > self.buf.capacity() {
                self.flush_buf()?;
            }
            if total_len >= self.buf.capacity() {
                self.panicked = true;
                let r = self.get_mut().write_vectored(bufs);
                self.panicked = false;
                r
            } else {
                // SAFETY: We checked whether or not the spare capacity was large enough above. If
                // it was, then we're safe already. If it wasn't, we flushed, making sufficient
                // room for any input <= the buffer size, which includes this input.
                unsafe {
                    bufs.iter().for_each(|b| self.write_to_buffer_unchecked(b));
                };

                Ok(total_len)
            }
        } else {
            let mut iter = bufs.iter();
            let mut total_written = if let Some(buf) = iter.by_ref().find(|&buf| !buf.is_empty()) {
                // This is the first non-empty slice to write, so if it does
                // not fit in the buffer, we still get to flush and proceed.
                if self.buf.len() + buf.len() > self.buf.capacity() {
                    self.flush_buf()?;
                }
                if buf.len() >= self.buf.capacity() {
                    // The slice is at least as large as the buffering capacity,
                    // so it's better to write it directly, bypassing the buffer.
                    self.panicked = true;
                    let r = self.get_mut().write(buf);
                    self.panicked = false;
                    return r;
                } else {
                    // SAFETY: We checked whether or not the spare capacity was large enough above.
                    // If it was, then we're safe already. If it wasn't, we flushed, making
                    // sufficient room for any input <= the buffer size, which includes this input.
                    unsafe {
                        self.write_to_buffer_unchecked(buf);
                    }

                    buf.len()
                }
            } else {
                return Ok(0);
            };
            debug_assert!(total_written != 0);
            for buf in iter {
                if self.buf.len() + buf.len() <= self.buf.capacity() {
                    // SAFETY: safe by above conditional.
                    unsafe {
                        self.write_to_buffer_unchecked(buf);
                    }

                    total_written += buf.len();
                } else {
                    break;
                }
            }
            Ok(total_written)
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buf().and_then(|()| self.get_mut().flush())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: Write> fmt::Debug for BufWriter<W>
where
    W: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("BufWriter")
            .field("writer", &self.inner.as_ref().unwrap())
            .field("buffer", &format_args!("{}/{}", self.buf.len(), self.buf.capacity()))
            .finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: Write + Seek> Seek for BufWriter<W> {
    /// Seek to the offset, in bytes, in the underlying writer.
    ///
    /// Seeking always writes out the internal buffer before seeking.
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.flush_buf()?;
        self.get_mut().seek(pos)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: Write> Drop for BufWriter<W> {
    fn drop(&mut self) {
        if self.inner.is_some() && !self.panicked {
            // dtors should not panic, so we ignore a failed flush
            let _r = self.flush_buf();
        }
    }
}
