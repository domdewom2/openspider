//! `os-export` – CSV, XLSX, Bulk-Exporte, Reports und Sitemap-Generator.
//!
//! Modulgerüst nach M1-Plan 3.6 (P00b). Formate, Dateinamen und Ausgabeordner: P08; Pipeline, Tab- und
//! Detail-Export, Links-Bulk-Exporte: P16; Reports, Issues-Export, Sitemap-Generator: P17.

pub mod catalog;
pub mod compare;
pub mod csv;
pub mod detail;
pub mod detail_export;
pub mod error;
pub mod fs_paths;
pub mod links_export;
pub mod naming;
pub mod output;
pub mod pipeline;
pub mod reports;
pub mod segment;
pub mod sitemap_gen;
pub mod tab_export;
pub mod target_gsheet;
pub mod timeseries;
pub mod types;
pub mod xlsx;

pub use catalog::{ExportKind, ExportRef};
pub use detail::export_detail;
pub use error::ExportError;
pub use output::open_writer;
pub use pipeline::export_bulk;
pub use reports::save_report;
pub use sitemap_gen::generate_sitemap;
pub use tab_export::export_tab;
pub use types::{
    DetailKind, ExportFileInfo, Format, ReportResult, SitemapOptions, SitemapReport, TableWriter,
    WriterOptions,
};
