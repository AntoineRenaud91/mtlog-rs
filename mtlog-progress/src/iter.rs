use crate::LogProgressBar;

/// A wrapper around an iterator that displays progress using `LogProgressBar`.
/// This struct is created by calling `.progress()` or `.progress_with()` on
/// an iterator.
pub struct LogProgressIterator<I> {
    inner: I,
    progress: LogProgressBar,
}

impl<I> LogProgressIterator<I> {
    fn new(inner: I, progress: LogProgressBar) -> Self {
        Self { inner, progress }
    }
}

impl<I: Iterator> Iterator for LogProgressIterator<I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            Some(item) => {
                self.progress.inc(1);
                Some(item)
            }
            None => {
                self.progress.finish();
                None
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<I: ExactSizeIterator> ExactSizeIterator for LogProgressIterator<I> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<I: DoubleEndedIterator> DoubleEndedIterator for LogProgressIterator<I> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self.inner.next_back() {
            Some(item) => {
                self.progress.inc(1);
                Some(item)
            }
            None => {
                self.progress.finish();
                None
            }
        }
    }
}

/// Extension trait for wrapping iterators with progress tracking.
///
/// This trait is implemented for all iterators and provides the `.progress_with()` method
/// which requires manually specifying the total length.
///
/// # Examples
///
/// ```
/// use mtlog_progress::ProgressIteratorExt;
///
/// vec![1, 2, 3, 4, 5]
///     .into_iter()
///     .progress_with(5, "Processing items")
///     .for_each(|item| {
///         // Work with item
///     });
/// ```
pub trait ProgressIteratorExt: Iterator + Sized {
    /// Wrap this iterator with progress tracking.
    ///
    /// # Arguments
    ///
    /// * `len` - The total number of items in the iterator
    /// * `name` - The name to display for this progress bar
    ///
    /// # Examples
    ///
    /// ```
    /// use mtlog_progress::ProgressIteratorExt;
    ///
    /// std::iter::repeat(42)
    ///     .take(100)
    ///     .progress_with(100, "Repeated values")
    ///     .for_each(|_| { /* work */ });
    /// ```
    fn progress_with(self, len: usize, name: &str) -> LogProgressIterator<Self> {
        let progress = LogProgressBar::new(len, name);
        LogProgressIterator::new(self, progress)
    }

    /// Wrap this `ExactSizeIterator` with automatic length detection.
    ///
    /// # Examples
    ///
    /// ```
    /// use mtlog_progress::ProgressIteratorExt;
    ///
    /// (0..100)
    ///     .progress("Counting")
    ///     .for_each(|n| {
    ///         // Work with n
    ///     });
    /// ```
    fn progress(self, name: &str) -> LogProgressIterator<Self>
    where
        Self: ExactSizeIterator,
    {
        let len = self.len();
        let progress = LogProgressBar::new(len, name);
        LogProgressIterator::new(self, progress)
    }
}

// Blanket implementation for all iterators
impl<I: Iterator> ProgressIteratorExt for I {}
