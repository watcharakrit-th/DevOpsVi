//! Logs as JSON, one object per line.

use tracing::{Level, Subscriber};
use tracing_subscriber::fmt::MakeWriter;

/// `main` writes to stdout; tests pass a buffer.
pub fn json_subscriber<W>(writer: W) -> Box<dyn Subscriber + Send + Sync>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    Box::new(
        tracing_subscriber::fmt()
            .json()
            // Event fields at the top level ({"message":"request","route":…})
            // rather than nested under "fields".
            .flatten_event(true)
            .with_current_span(false)
            .with_span_list(false)
            .with_max_level(Level::INFO)
            .with_writer(writer)
            .finish(),
    )
}
