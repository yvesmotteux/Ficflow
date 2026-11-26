use super::fanfiction::FanfictionOps;
use super::inbox::InboxOps;
use super::series::SeriesOps;
use super::shelf::ShelfOps;

// Aggregate trait so the composition root (factory, interface, executor) can
// depend on a single "does fic and shelf ops" reference instead of passing
// the same object twice as two separate trait objects. Application functions
// still take the specific trait they need; trait upcasting coerces
// `&dyn Repository` to `&dyn FanfictionOps` / `&dyn ShelfOps` at call sites.
pub trait Repository: FanfictionOps + ShelfOps + InboxOps + SeriesOps {}

impl<T: FanfictionOps + ShelfOps + InboxOps + SeriesOps> Repository for T {}
