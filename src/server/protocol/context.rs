use serde::Serialize;

const AS2_CONTEXT: &str = "https://www.w3.org/ns/activitystreams";
pub const FEDISPORT_CONTEXT: &str = "https://fedisport.github.io/vocabulary/context.jsonld";

/// Wraps an AP activity or object and serializes `@context` as an array
/// containing both the ActivityStreams 2.0 and the canonical fedisport
/// vocabulary context URL.
#[derive(Debug, Clone, Serialize)]
pub struct WithFedisportContext<T: Serialize> {
    #[serde(rename = "@context")]
    pub context: [&'static str; 2],
    #[serde(flatten)]
    pub inner: T,
}

impl<T: Serialize> WithFedisportContext<T> {
    pub fn new(inner: T) -> Self {
        Self {
            context: [AS2_CONTEXT, FEDISPORT_CONTEXT],
            inner,
        }
    }
}
