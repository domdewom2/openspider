//! robots.txt: eigener Parser nach RFC 9309 und der `RobotsGate` (Abruf je Origin, Fehlerpolitik, Modi).
//!
//! * [`parse`] / [`RobotsTxt`]: Parser und Regelauswertung (CRAWL-098), siehe `parser.rs`.
//! * [`RobotsGate`]: Abruf je Origin genau einmal, Modi Respektieren/Ignorieren/Ignorieren-aber-melden
//!   (CRAWL-099) und Fehlerpolitik (CRAWL-100), siehe `gate.rs`.

mod gate;
mod parser;
mod pattern;

pub use gate::{RobotsCheck, RobotsDecision, RobotsGate, RobotsGateConfig};
pub use parser::{parse, RobotsMatch, RobotsTxt};
