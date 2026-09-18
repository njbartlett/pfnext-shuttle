// Bootstrap's JavaScript is still loaded as a classic script by
// templates/base_layout.html.tera (shared with the legacy pages), so it is a
// global here rather than an import. Importing it from npm as well would
// initialise every data-api component twice.
declare const bootstrap: typeof import('bootstrap')
