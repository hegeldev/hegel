/// The span label for a generator identified by a name.
///
/// A label is an opaque value identifying a generator to the engine, which
/// treats two spans with the same label as coming from the same generator
/// (see [`Generator::label`](super::Generator::label)). This asks the engine
/// for the label of a name (`hegel_label_from_name`), so every frontend
/// derives the same label from the same name. Compute it once and keep it,
/// for example in a [`LazyLock`](std::sync::LazyLock):
///
/// ```
/// use hegel::generators as gs;
/// use std::sync::LazyLock;
///
/// static PAIRS_LABEL: LazyLock<u64> = LazyLock::new(|| gs::label_from_name("mycrate.pairs"));
/// ```
///
/// A name only has to be stable and unique to its generator; prefixing it
/// with the crate's name keeps it clear of Hegel's own `hegel.<kind>` names.
pub fn label_from_name(name: &str) -> u64 {
    crate::ffi::label_from_name(name)
}

/// The span label for a generator built from other generators.
///
/// Combines the given labels, in order, into one, through the engine's
/// `hegel_label_combine`. Pass the generator's own label (from
/// [`label_from_name`]) first and its components' labels after it, so that
/// a pair of integers and a pair of strings get different labels while
/// every pair of integers gets the same one. Combine once, when the
/// generator is built, and keep the result: `label` is asked for on every
/// draw.
///
/// ```
/// use hegel::generators::{self as gs, Generator};
/// use std::sync::LazyLock;
///
/// static PAIRS_LABEL: LazyLock<u64> = LazyLock::new(|| gs::label_from_name("mycrate.pairs"));
///
/// struct Pairs<G> {
///     inner: G,
///     label: u64,
/// }
///
/// fn pairs<T, G: Generator<T>>(inner: G) -> Pairs<G> {
///     let label = gs::combine_labels(&[*PAIRS_LABEL, inner.label()]);
///     Pairs { inner, label }
/// }
///
/// impl<T, G: Generator<T>> Generator<(T, T)> for Pairs<G> {
///     fn label(&self) -> u64 {
///         self.label
///     }
///
///     fn do_draw(&self, tc: &hegel::TestCase) -> (T, T) {
///         (tc.draw_silent(&self.inner), tc.draw_silent(&self.inner))
///     }
/// }
/// ```
///
/// Combining is order-sensitive, and combining a single label does not
/// return it unchanged.
pub fn combine_labels(labels: &[u64]) -> u64 {
    crate::ffi::combine_labels(labels)
}

#[cfg(test)]
#[path = "../../tests/embedded/generators/labels_tests.rs"]
mod tests;
