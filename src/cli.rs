//! Port of `CommandLineParser` / `Runner` from VsixCommandLine.dll.

use std::collections::HashMap;

use crate::logger::{format_net, Logger};
use crate::messages as msg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgumentType {
    Boolean,
    NonBoolean,
}

#[derive(Debug, Clone)]
pub struct Argument {
    pub name: &'static str,
    pub kind: ArgumentType,
    pub help: &'static str,
    pub required: bool,
    pub hidden: bool,
}

impl Argument {
    pub const fn new(name: &'static str, kind: ArgumentType, help: &'static str) -> Self {
        Self { name, kind, help, required: false, hidden: false }
    }
    pub const fn required(mut self) -> Self {
        self.required = true;
        self
    }
    pub const fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgValue {
    Flag,
    Value(String),
}

/// Selected arguments keyed by canonical argument name.
#[derive(Debug, Default, Clone)]
pub struct Args(HashMap<&'static str, ArgValue>);

impl Args {
    pub fn get(&self, name: &str) -> Option<&str> {
        match self.0.get(name) {
            Some(ArgValue::Value(v)) => Some(v.as_str()),
            _ => None,
        }
    }
    pub fn flag(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }
    pub fn contains(&self, name: &str) -> bool {
        self.0.contains_key(name)
    }
    pub fn insert(&mut self, name: &'static str, v: ArgValue) {
        self.0.insert(name, v);
    }
}

pub trait Command {
    fn arguments(&self) -> Vec<Argument>;
    fn help_text(&self) -> &'static str;
    fn validate(&self, _args: &Args, _log: &mut Logger) -> i32 {
        0
    }
    fn execute(&self, args: &Args, log: &mut Logger) -> i32;
}

pub struct Parser {
    commands: Vec<(&'static str, Box<dyn Command>)>,
}

const HELP: &str = "help";
const VERSION: &str = "version";

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    pub fn new() -> Self {
        Self { commands: Vec::new() }
    }

    pub fn add(&mut self, name: &'static str, cmd: Box<dyn Command>) {
        assert!(
            name != HELP && name != VERSION && self.find(name).is_none(),
            "Cannot add a command to the parser that has already been added."
        );
        self.commands.push((name, cmd));
    }

    fn find(&self, name: &str) -> Option<&dyn Command> {
        self.commands
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, c)| c.as_ref())
    }

    pub fn version_string() -> String {
        format!("{} {}", msg::TOOL_NAME, env!("CARGO_PKG_VERSION"))
    }

    pub fn usage(&self, selected: Option<&dyn Command>, error: Option<&str>) -> String {
        let mut s = String::new();
        s.push_str(&Self::version_string());
        s.push('\n');
        s.push_str(msg::COPYRIGHT);
        s.push_str("\n\n");
        if let Some(e) = error.filter(|e| !e.is_empty()) {
            s.push_str(e);
            s.push('\n');
        }
        match selected {
            None => {
                let mut list: Vec<(&str, &str)> = self
                    .commands
                    .iter()
                    .map(|(n, c)| (*n, c.help_text()))
                    .collect();
                list.push((HELP, msg::HELP_HELP_TEXT));
                list.push((VERSION, msg::VERSION_HELP_TEXT));
                list.sort_by_key(|(n, _)| n.to_lowercase());
                for (n, h) in list {
                    s.push_str(&format!("  {n:<30}{h}\n\n"));
                }
            }
            Some(cmd) => {
                let mut args: Vec<Argument> =
                    cmd.arguments().into_iter().filter(|a| !a.hidden).collect();
                args.sort_by_key(|a| (!a.required, a.name.to_ascii_lowercase()));
                for a in args {
                    let help = if a.required {
                        format_net(msg::REQUIRED_ARGUMENT_FORMAT, &[&a.help])
                    } else {
                        a.help.to_string()
                    };
                    s.push_str(&format!("  -{:<30}{help}\n\n", a.name));
                }
            }
        }
        s
    }

    /// Parses `args` (without program name). On error returns the selected command (if any) and message.
    pub fn parse<'a>(
        &'a self,
        args: &[String],
    ) -> Result<(&'a str, Args), (Option<&'a str>, String)> {
        let Some(name) = args.first() else {
            return Err((None, msg::MISSING_COMMAND.to_string()));
        };
        let (cmd_name, arguments): (&str, Vec<Argument>) = match name.as_str() {
            HELP => (HELP, Vec::new()),
            VERSION => (VERSION, Vec::new()),
            n => match self.commands.iter().find(|(cn, _)| *cn == n) {
                Some((cn, c)) => (cn, c.arguments()),
                None => return Err((None, format_net(msg::INVALID_COMMAND, &[name]))),
            },
        };
        let fail = |m: String| Err((Some(cmd_name), m));
        let mut selected = Args::default();
        let mut i = 1;
        while i < args.len() {
            let text = &args[i];
            if !text.starts_with('-') && !text.starts_with('/') {
                return fail(format_net(msg::INVALID_ARGUMENT, &[text]));
            }
            let arg_name = &text[1..];
            let Some(arg) = arguments.iter().find(|a| a.name.eq_ignore_ascii_case(arg_name)) else {
                return fail(format_net(msg::INVALID_ARGUMENT, &[text]));
            };
            if arg.kind == ArgumentType::Boolean {
                selected.insert(arg.name, ArgValue::Flag);
                i += 1;
                continue;
            }
            i += 1;
            match args.get(i) {
                Some(v) if !v.starts_with('-') && !v.starts_with('/') => {
                    selected.insert(arg.name, ArgValue::Value(v.clone()));
                }
                _ => return fail(format_net(msg::ARGUMENT_MISSING_VALUE, &[text])),
            }
            i += 1;
        }
        if let Some(missing) = arguments
            .iter()
            .find(|a| a.required && !selected.contains(a.name))
        {
            return fail(format_net(msg::MISSING_REQUIRED_ARGUMENT, &[&missing.name]));
        }
        Ok((cmd_name, selected))
    }

    /// Port of `Runner.Run`. Returns the process exit code.
    pub fn run(&self, args: &[String], log: &mut Logger, stderr: &mut dyn std::io::Write) -> i32 {
        let (name, selected) = match self.parse(args) {
            Ok(v) => v,
            Err((cmd, e)) => {
                let cmd = cmd.and_then(|n| self.find(n));
                let _ = writeln!(stderr, "{}", self.usage(cmd, Some(&e)));
                return -1;
            }
        };
        match name {
            HELP => {
                log.message(&self.usage(None, None));
                0
            }
            VERSION => {
                log.message(&Self::version_string());
                0
            }
            _ => {
                let cmd = self.find(name).expect("parsed command exists");
                let code = cmd.validate(&selected, log);
                if code != 0 {
                    return code;
                }
                cmd.execute(&selected, log)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logger::test_support::{capture, Buf};
    use ArgumentType::*;

    struct Dummy;
    impl Command for Dummy {
        fn arguments(&self) -> Vec<Argument> {
            vec![
                Argument::new("opt", NonBoolean, "Optional."),
                Argument::new("name", NonBoolean, "The name.").required(),
                Argument::new("force", Boolean, "Force it."),
                Argument::new("secret", NonBoolean, "Hidden.").hidden(),
            ]
        }
        fn help_text(&self) -> &'static str {
            "Does things."
        }
        fn execute(&self, args: &Args, log: &mut Logger) -> i32 {
            log.message(&format!(
                "name={} force={} opt={:?}",
                args.get("name").unwrap(),
                args.flag("force"),
                args.get("opt")
            ));
            0
        }
    }

    fn parser() -> Parser {
        let mut p = Parser::new();
        p.add("doIt", Box::new(Dummy));
        p
    }

    fn sv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    fn err(a: &[&str]) -> String {
        parser().parse(&sv(a)).err().unwrap().1
    }

    #[test]
    fn parses_args_case_insensitive_with_both_prefixes() {
        let p = parser();
        let (n, a) = p.parse(&sv(&["doIt", "-NAME", "x", "/Force", "-opt", "y"])).unwrap();
        assert_eq!(n, "doIt");
        assert_eq!(a.get("name"), Some("x"));
        assert!(a.flag("force"));
        assert_eq!(a.get("opt"), Some("y"));
    }

    #[test]
    fn parse_errors() {
        assert_eq!(err(&[]), "A command is required to run the tool.");
        assert_eq!(err(&["doit"]), "The 'doit' command is invalid.");
        assert_eq!(err(&["doIt", "x"]), "'x' is an invalid argument.");
        assert_eq!(err(&["doIt", "-nope"]), "'-nope' is an invalid argument.");
        assert_eq!(err(&["doIt", "-name"]), "'-name' requires a value.");
        assert_eq!(err(&["doIt", "-name", "-force"]), "'-name' requires a value.");
        assert_eq!(err(&["doIt", "-force"]), "Missing required argument: 'name'.");
    }

    #[test]
    fn command_usage_lists_required_first_and_skips_hidden() {
        let p = parser();
        let u = p.usage(Some(&Dummy), Some("boom"));
        let expected = format!(
            "{}\n{}\n\nboom\n  -{:<30}Required. The name.\n\n  -{:<30}Force it.\n\n  -{:<30}Optional.\n\n",
            Parser::version_string(),
            msg::COPYRIGHT,
            "name",
            "force",
            "opt"
        );
        assert_eq!(u, expected);
    }

    #[test]
    fn general_usage_lists_sorted_commands() {
        let u = parser().usage(None, None);
        let lines: Vec<&str> = u.lines().filter(|l| l.starts_with("  ")).collect();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("  doIt"));
        assert!(lines[1].starts_with("  help"));
        assert!(lines[2].starts_with("  version"));
    }

    #[test]
    fn run_executes_and_reports_parse_errors() {
        let p = parser();
        let (mut log, out, _) = capture();
        let mut se = Buf::default();
        assert_eq!(p.run(&sv(&["doIt", "-name", "a"]), &mut log, &mut se), 0);
        assert_eq!(out.text(), "name=a force=false opt=None\n");
        assert_eq!(p.run(&sv(&["bad"]), &mut log, &mut se), -1);
        assert!(se.text().contains("The 'bad' command is invalid."));
    }
}
