//! Command-line arguments accepted by the C# client, parsed the same lenient
//! way (`-host=`, `-cfg=`, `-lang:`, `-allowMultiple` substring match).

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Args {
    /// `-host=ip[:port]`: connect automatically after start.
    pub host: Option<String>,
    /// `-cfg=<dir>`: configuration directory (default: working directory).
    pub cfg_dir: Option<String>,
    /// `-lang:<culture>`: accepted and ignored for now.
    pub lang: Option<String>,
    /// `-allowMultiple` anywhere in an argument.
    pub allow_multiple: bool,
}

impl Args {
    pub fn parse<I: IntoIterator<Item = String>>(args: I) -> Self {
        let mut out = Args::default();
        for raw in args {
            let arg = raw.trim();
            if let Some(v) = arg.strip_prefix("-host=") {
                out.host = Some(v.trim().to_string());
            } else if let Some(v) = arg.strip_prefix("-cfg=") {
                out.cfg_dir = Some(v.trim().to_string());
            } else if let Some(v) = arg.strip_prefix("-lang:") {
                out.lang = Some(v.trim().to_string());
            }
            if arg.contains("-allowMultiple") {
                out.allow_multiple = true;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_like_csharp() {
        let a = Args::parse(
            [
                "-host=1.2.3.4:5002",
                "-cfg=C:\\srs\\",
                "-allowMultiple",
                "-lang:zh",
            ]
            .map(String::from),
        );
        assert_eq!(a.host.as_deref(), Some("1.2.3.4:5002"));
        assert_eq!(a.cfg_dir.as_deref(), Some("C:\\srs\\"));
        assert_eq!(a.lang.as_deref(), Some("zh"));
        assert!(a.allow_multiple);
        assert_eq!(Args::parse(Vec::<String>::new()), Args::default());
    }
}
