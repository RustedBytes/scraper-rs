use std::borrow::Cow;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

pub(crate) const DEFAULT_MAX_PARSE_BYTES: usize = 1_073_741_824; // 1 GiB

pub(crate) fn ensure_within_size_limit(
    html: &str,
    max_size_bytes: usize,
    truncate_on_limit: bool,
) -> PyResult<Cow<'_, str>> {
    let len_bytes = html.len();
    if len_bytes > max_size_bytes {
        if truncate_on_limit {
            // Truncate to max_size_bytes, ensuring we don't split a UTF-8 character
            let mut truncate_at = max_size_bytes;
            while truncate_at > 0 && !html.is_char_boundary(truncate_at) {
                truncate_at -= 1;
            }
            return Ok(Cow::Owned(html[..truncate_at].to_string()));
        }
        return Err(PyValueError::new_err(format!(
            "HTML document is too large to parse: {len_bytes} bytes exceeds max_size_bytes={max_size_bytes}"
        )));
    }

    Ok(Cow::Borrowed(html))
}
