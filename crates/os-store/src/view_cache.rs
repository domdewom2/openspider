//! `view_cache`: reiner, jederzeit verwerfbarer Cache der View-Sitzungen (K-1a, Schema v3).
//!
//! Der Store speichert den BLOB opak (`data`); Aufbau, Serialisierung (Roaring-Bitmaps, Sortierpermutationen) und
//! Invalidierung über `step_stamp`/`built_seq` gehören M2-K01.

// STUB(M2-K01)
