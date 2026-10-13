//! Issue-Klassifizierung (UI-063/UI-064).

use crate::enums::string_enum;

string_enum! {
    /// Art eines Befunds.
    pub enum IssueType {
        Issue => "Issue",
        Warning => "Warning",
        Opportunity => "Opportunity",
    }
}

string_enum! {
    /// Priorität eines Befunds.
    pub enum IssuePriority {
        High => "High",
        Medium => "Medium",
        Low => "Low",
    }
}
