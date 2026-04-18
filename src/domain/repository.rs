use super::fanfiction::FanfictionOps;
use super::shelf::ShelfOps;

// Aggregate trait so the composition root (factory, interface, executor) can
// depend on a single "does fic and shelf ops" reference instead of passing
// the same object twice as two separate trait objects. Application functions
// still take the specific trait they need; trait upcasting coerces
// `&dyn Repository` to `&dyn FanfictionOps` / `&dyn ShelfOps` at call sites.
pub trait Repository: FanfictionOps + ShelfOps {}

impl<T: FanfictionOps + ShelfOps> Repository for T {}
