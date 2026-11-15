use serde_derive::{Deserialize, Serialize};

mod client;
mod server;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(all(not(target_env = "ohos"), any(target_os = "windows", target_os = "linux")))]
mod win_linux;

#[cfg(target_os = "windows")]
use windows::create_event_loop;
#[cfg(target_os = "macos")]
use macos::create_event_loop;
#[cfg(all(target_os = "linux", not(target_env = "ohos")))]
pub use linux::is_supported;

pub use client::*;
pub use server::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum CustomEvent {
    Cursor(Cursor),
    Clear,
    Exit,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t")]
pub struct Cursor {
    pub x: f32,
    pub y: f32,
    pub argb: u32,
    pub btns: i32,
    pub text: String,
}
