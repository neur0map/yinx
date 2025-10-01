//! Yinx - Intelligent Penetration Testing Companion
//!
//! A background CLI daemon that captures terminal activity during penetration testing,
//! intelligently filters noise, semantically indexes findings, and provides instant
//! retrieval with optional AI assistance.

pub mod cli;
pub mod config;
pub mod daemon;
pub mod error;
pub mod patterns;
pub mod session;
pub mod storage;

pub use error::{Result, YinxError};
