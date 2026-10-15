//! Client settings files, compatible with the C# client so that a user can
//! switch between the two clients without losing anything.
//!
//! - [`ini`]: parser/writer reproducing SharpConfig 4.1 (`Name=Value`,
//!   `[Section]`, `#`/`;` comments, `{a,b}` arrays, quoting of values with
//!   spaces, `True`/`False` booleans).
//! - [`keys`]: the enums whose *names* are the keys on disk.
//! - [`global`]: `global.cfg` (`GlobalSettingsStore`).
//! - [`profile`]: `<name>.cfg` (`ProfileSettingsStore`) including input bindings.

pub mod favourites;
pub mod global;
pub mod ini;
pub mod keys;
pub mod profile;
pub mod profiles;

pub use favourites::{FavouriteStore, ServerAddress};
pub use global::GlobalSettings;
pub use ini::{Document, ParseError};
pub use keys::{GlobalSettingsKey, InputBinding, ProfileSettingsKey};
pub use profile::{InputDevice, ProfileSettings};
