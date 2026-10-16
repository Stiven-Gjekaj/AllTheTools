//! `att edid`: show what each connected display says about itself: the
//! maker, the model, the serial number, the size and the native mode.
//!
//! Linux has `edid-decode`, which reads /sys/class/drm. macOS and Windows
//! keep the EDID of a display deep in the IOKit registry and in the
//! registry of Windows, and have no command that reads it.

pub mod parse;
