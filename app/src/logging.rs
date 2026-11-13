//! JSON logs. Stub for the failing tests.

use tracing::Subscriber;
use tracing_subscriber::fmt::MakeWriter;

pub fn json_subscriber<W>(_writer: W) -> Box<dyn Subscriber + Send + Sync>
where
    W: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    todo!()
}
