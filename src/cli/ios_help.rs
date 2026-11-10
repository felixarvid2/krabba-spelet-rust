//! Hjälptexter för ? (HELP, SHOW_HELP och PARAM_HELP i ios.js, plus tilläggen längre ner i filen).

/// Hjälptext för ett nyckelord. in_show = raden börjar med show.
pub fn help_text(k: &str, in_show: bool) -> &'static str {
    let _ = (k, in_show);
    ""
}

/// Hjälp för parametertyper: ("A.B.C.D", "IP address") osv.
pub fn param_help(ty: &str) -> Option<(&'static str, &'static str)> {
    let _ = ty;
    None
}
