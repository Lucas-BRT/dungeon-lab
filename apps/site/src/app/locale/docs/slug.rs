pub mod _page;

use topcoat::router::module_param;

// The page being read: /en/docs/model, /pt-BR/docs/roadmap, …
module_param!(pub slug);
