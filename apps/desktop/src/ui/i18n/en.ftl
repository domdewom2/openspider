## Shared UI strings: dialogs, errors, empty states, colors, shortcuts, menu titles

ui-dialog-close = Close
ui-dialog-cancel = Cancel
ui-dialog-ok = OK
ui-dialog-apply = Apply
ui-dialog-reset = Reset
ui-dismiss = Dismiss
ui-clear-search = Clear search
ui-region-reload = Reload area
ui-region-failed = This area could not be displayed.
ui-search-placeholder = Search
ui-toasts = Notifications

## Errors (error classes of the core, messageKey of CmdError)
err-cancelled = The operation was cancelled.
err-stale = The data is out of date.
err-internal = An unexpected error occurred.
err-unsupported = This function is not available yet.
err-bridge-closed = The connection to the core was closed.
err-code-invalidinput = The input is invalid.
err-code-notfound = The item was not found.
err-code-stale = The data is out of date.
err-code-cancelled = The operation was cancelled.
err-code-busy = The core is busy. Please try again.
err-code-timeout = The operation timed out.
err-code-io = A file operation failed.
err-code-permission = Permission denied.
err-code-unsupported = This function is not available yet.
err-code-nocrawl = No crawl is open.
err-code-crawlrunning = A crawl is running.
err-code-store = The crawl file could not be read.
err-code-engine = The crawl engine reported an error.
err-code-internal = An unexpected error occurred.

## Empty and prerequisite states (UI-016)
empty-no-selection = Select a row in the table above to see its details.
empty-mode-unsupported = This view is not available in the current mode.
empty-needs-js-rendering = Only available in JavaScript rendering mode. Switch the rendering mode in the settings.
empty-feature-disabled = { $arg ->
    [cookies] Cookies are not collected. Turn on cookie collection in the settings.
    [http_headers] HTTP headers are not stored. Turn on header storage in the settings.
    [accessibility] The accessibility check is turned off. Turn it on in the settings.
    [spelling] Spelling and grammar checks are turned off. Turn them on in the settings.
    [near_duplicates] Near-duplicate detection is turned off. Turn it on in the settings.
    [structured_data] Structured data extraction is turned off. Turn it on in the settings.
   *[other] The function "{ $arg }" is turned off. Turn it on in the settings.
}
empty-html-not-stored = The HTML source is not stored. Turn on "Store HTML" in the settings.
empty-rendered-html-not-stored = The rendered HTML is not stored. Turn on "Store rendered HTML" in the settings.
empty-api-not-connected = { $arg ->
    [gsc] Search Console is not connected. Connect it to see this data.
    [ga4] Google Analytics 4 is not connected. Connect it to see this data.
    [psi] PageSpeed Insights is not connected. Connect it to see this data.
   *[other] The data source "{ $arg }" is not connected. Connect it to see this data.
}
empty-needs-crawl-analysis = The crawl analysis step "{ $arg }" has not run yet or is out of date. Start the crawl analysis.
empty-no-data-yet = No data yet. Start a crawl or open a crawl file.
empty-no-match = No rows match the current filter.
empty-not-applicable = There is no data of this kind for this URL type.
empty-action-open-setting = Open setting
empty-action-start-analysis = Start crawl analysis
empty-action-clear-search = Clear search
empty-action-connect-api = Connect

## Colors
color-green = Green
color-blue = Blue
color-teal = Teal
color-violet = Violet
color-pink = Pink
color-red = Red
color-orange = Orange
color-olive = Olive
color-custom = Custom color
color-invalid = Enter a color as #rrggbb.
color-low-contrast = Contrast { $ratio }:1 against the background is below 3:1. Selections get an additional outline.

## Menu titles (UI-002)
menu-app = OpenSpider
menu-file = File
menu-edit = Edit
menu-config = Configuration
menu-mode = Mode
menu-bulk-export = Bulk Export
menu-reports = Reports
menu-sitemaps = Sitemaps
menu-visualisations = Visualisations
menu-crawl-analysis = Crawl Analysis
menu-view = View
menu-mcp = MCP
menu-window = Window
menu-help = Help

## Keyboard shortcuts (UI-113), labels shown in menus and in the help dialog
shortcut-crawl-start = Start or resume crawl
shortcut-crawl-pause = Pause crawl
shortcut-crawl-new = New crawl
shortcut-crawl-open = Open crawl
shortcut-crawl-save-as = Save crawl as…
shortcut-app-settings = Settings
shortcut-config-spider = Spider configuration
shortcut-config-diff = Configuration diff
shortcut-search-focus = Focus search field
shortcut-search-clear = Clear search
shortcut-table-copy = Copy
shortcut-table-select-all = Select all
shortcut-export-view = Export current view
shortcut-toptab-next = Next top tab
shortcut-toptab-prev = Previous top tab
shortcut-toptab-goto-1 = Top tab 1
shortcut-toptab-goto-2 = Top tab 2
shortcut-toptab-goto-3 = Top tab 3
shortcut-toptab-goto-4 = Top tab 4
shortcut-toptab-goto-5 = Top tab 5
shortcut-toptab-goto-6 = Top tab 6
shortcut-toptab-goto-7 = Top tab 7
shortcut-toptab-goto-8 = Top tab 8
shortcut-toptab-goto-9 = Top tab 9
shortcut-filter-open = Open filter dropdown
shortcut-focus-next = Move focus: table, details, sidebar
shortcut-focus-prev = Move focus backwards
shortcut-view-toggle-detail = Show or hide details
shortcut-view-toggle-sidebar = Show or hide sidebar
shortcut-nav-back = Back
shortcut-nav-forward = Forward
shortcut-browser-open = Open in browser
shortcut-crawl-recrawl = Re-crawl
shortcut-table-remove = Remove
shortcut-analysis-start = Start crawl analysis
shortcut-source-next-diff = Next difference
shortcut-source-prev-diff = Previous difference
shortcut-zoom-in = Zoom in
shortcut-zoom-out = Zoom out
shortcut-zoom-reset = Reset zoom to 100 %
shortcut-context-menu = Context menu
