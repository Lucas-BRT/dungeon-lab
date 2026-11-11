pub mod _page;

use topcoat::router::module_param;

// The page being read: /docs/model, /docs/roadmap, …
module_param!(pub slug);
