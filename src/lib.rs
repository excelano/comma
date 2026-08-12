// Comma's document model, deliberately free of the toolkit so it can be tested
// on its own and read without knowing anything about widgets.
//
// One module reaches past that line and says why it does: `value` asks GLib to
// collate text, because what order words go in belongs to the reader's own
// language. Everything else here is bytes and strings.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

pub mod document;
pub mod export;
pub mod filter;
pub mod search;
pub mod value;
