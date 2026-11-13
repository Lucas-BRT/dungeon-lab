//! Every address that matches no page: the leaf chamber, with a 404.

pub mod _page;

topcoat::router::segment!(kind: CatchAll, rename: "path");
