// Handing the file, or the folder it sits in, to something outside the window.
//
// Comma edits one file and has no project root, so there is only ever one
// folder a terminal could open in and only ever one file to pass on. That is
// what makes these menu items rather than a pane along the bottom: an editor
// holding several files has to keep answering where it is, and Comma answered
// once when the file was opened.
//
// Everything Comma starts outside itself is written down here, so what leaves
// the process is in one place and can be read without going looking.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::gio;
use gtk::glib;

use comma::document::Dialect;

use super::CommaWindow;

/// A terminal Comma knows how to start, in the order it looks for one.
///
/// The terminals GNOME ships come first, because Comma is a GNOME application
/// and the terminal the desktop came with is the one its user is likeliest to
/// be at home in. The last two are what any Unix has: Debian's alternative,
/// which is whichever terminal that system chose, and then xterm.
struct Terminal {
    program: &'static str,
    /// The flag that starts it in a named folder, or None for one that has no
    /// such flag and opens wherever it was started from.
    ///
    /// Naming the folder is better than being started in it even where both
    /// work, because these are the terminals that hand the request to a server
    /// process: what the server does with the folder the request came from is
    /// its own business, and it has changed before.
    folder: Option<&'static str>,
    /// The flag a command to run in the new terminal follows. GNOME's take the
    /// rest of the line after `--`; the xterm lineage takes it after `-e`.
    command: &'static str,
}

const TERMINALS: [Terminal; 5] = [
    Terminal {
        program: "ptyxis",
        folder: Some("-d"),
        command: "--",
    },
    Terminal {
        program: "kgx",
        folder: Some("--working-directory"),
        command: "--",
    },
    Terminal {
        program: "gnome-terminal",
        folder: Some("--working-directory"),
        command: "--",
    },
    Terminal {
        program: "x-terminal-emulator",
        folder: None,
        command: "-e",
    },
    Terminal {
        program: "xterm",
        folder: None,
        command: "-e",
    },
];

/// A command-line tool Comma can hand the open file to.
///
/// A tool earns a place here by opening something to work in when it is given a
/// file and nothing else. xled and xql drop into prompts of their own, and nved
/// is the way back to the file as text, for when what is wrong is the reading
/// itself. A tool that answers a question and exits is not offered: there would
/// be nothing to do in the window that opened, and the terminal Comma can
/// already open is a better place to run it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tool {
    Xled,
    Xql,
    Nved,
}

impl Tool {
    /// The tools, in the order the menu offers them.
    pub(super) const ALL: [Tool; 3] = [Self::Xled, Self::Xql, Self::Nved];

    /// The program to look for, which is also what the menu calls it. A tool's
    /// name is its name in every language, so it is the one word here that is
    /// not translated.
    pub(super) fn program(self) -> &'static str {
        match self {
            Self::Xled => "xled",
            Self::Xql => "xql",
            Self::Nved => "nved",
        }
    }

    pub(super) fn action(self) -> &'static str {
        match self {
            Self::Xled => "open-with-xled",
            Self::Xql => "open-with-xql",
            Self::Nved => "open-with-nved",
        }
    }

    pub(super) fn label(self) -> String {
        gettext("Open with {}").replace("{}", self.program())
    }

    /// Whether the tool can read a file in this dialect.
    ///
    /// xled and xql end a record at a line break, so a file whose records are
    /// separated by the ASCII record separator reaches them as a single
    /// enormous row. nved is told which layer to read the file on and reads it
    /// as the file means it.
    pub(super) fn reads(self, dialect: Dialect) -> bool {
        match self {
            Self::Xled | Self::Xql => dialect != Dialect::unit_separator(),
            Self::Nved => true,
        }
    }
}

impl CommaWindow {
    /// Shows the file where it lives, in whatever the desktop uses to look at
    /// folders. It goes through the portal, so it is also the one of these that
    /// still means something for a file that was opened over the network.
    pub(super) fn open_containing_folder(&self) {
        let Some(file) = self.imp().file.borrow().clone() else {
            return;
        };

        let window = self.clone();
        gtk::FileLauncher::new(Some(&file)).open_containing_folder(
            Some(self),
            gio::Cancellable::NONE,
            move |result| {
                if result.is_err() {
                    window.say(&gettext("Could not open the folder."));
                }
            },
        );
    }

    pub(super) fn open_terminal(&self) {
        self.start(&[]);
    }

    /// Hands the file to a tool, after offering to save what is on screen.
    ///
    /// The tool reads the file from disk, so edits that have not been saved are
    /// edits it will not see. Opening the older bytes is a real answer — what
    /// is on disk is what the rest of the system can see — so it is offered
    /// rather than refused, and saving is what the button under the cursor
    /// already does.
    pub(super) fn open_with(&self, tool: Tool) {
        if !self.is_modified() {
            return self.start_tool(tool);
        }

        // Two things to fill in, so each says which it is. A sentence is
        // translated whole and another language may want them the other way
        // round, which two of the same mark could not survive.
        let message = gettext("“{file}” has unsaved changes. {tool} reads the file from disk.")
            .replace("{file}", &self.document_name())
            .replace("{tool}", tool.program());
        let dialog = adw::AlertDialog::new(Some(&gettext("Save Before Opening?")), Some(&message));
        dialog.add_response("cancel", &gettext("_Cancel"));
        dialog.add_response("open", &gettext("Open _Anyway"));
        dialog.add_response("save", &gettext("_Save"));
        dialog.set_response_appearance("save", adw::ResponseAppearance::Suggested);
        dialog.set_default_response(Some("save"));
        dialog.set_close_response("cancel");

        let window = self.clone();
        dialog.choose(Some(self), gio::Cancellable::NONE, move |response| {
            match response.as_str() {
                "open" => window.start_tool(tool),
                // Nothing is handed over on the strength of a save that did not
                // happen.
                "save" if window.save() => window.start_tool(tool),
                _ => {}
            }
        });
    }

    fn start_tool(&self, tool: Tool) {
        let Some(document) = self.imp().rows.document() else {
            return;
        };
        let Some(path) = self.imp().file.borrow().as_ref().and_then(gio::File::path) else {
            return;
        };

        let dialect = document.borrow().dialect();
        let header = self.imp().rows.header();
        self.start(&invocation(tool, dialect, header, &path.to_string_lossy()));
    }

    /// Opens a terminal in the file's folder, running `command` in it when
    /// there is one to run.
    fn start(&self, command: &[String]) {
        let Some(folder) = self.folder_path() else {
            return;
        };

        let asked_for = glib::getenv("TERMINAL").and_then(|name| name.into_string().ok());
        let Some(argv) = command_for(
            &folder,
            asked_for.as_deref(),
            &|name| glib::find_program_in_path(name).is_some(),
            command,
        ) else {
            return self.say(&gettext("No terminal application was found."));
        };

        // Set whether or not the terminal was also told in words: it is what
        // the ones with nothing to tell rely on, and it is what the command
        // running inside inherits either way.
        let launcher = gio::SubprocessLauncher::new(gio::SubprocessFlags::NONE);
        launcher.set_cwd(&folder);
        let argv: Vec<&std::ffi::OsStr> = argv.iter().map(|word| word.as_ref()).collect();
        if launcher.spawn(&argv).is_err() {
            self.say(&gettext("Could not open a terminal."));
        }
    }

    /// The folder the file is in, as somewhere a shell can stand. A file
    /// reached over the network has none, which is why this is what the menu
    /// item is greyed out by.
    pub(super) fn folder_path(&self) -> Option<String> {
        let file = self.imp().file.borrow().clone()?;
        let folder = file.parent()?.path()?;
        Some(folder.to_string_lossy().into_owned())
    }
}

/// Whether a terminal Comma started would reach the system the user is on.
///
/// Inside a sandbox it would not. There is no portal for asking the desktop for
/// a terminal, so the item is left out rather than offered and made to fail.
pub(super) fn terminals_reachable() -> bool {
    !std::path::Path::new("/.flatpak-info").exists()
}

/// The command that opens a terminal in `folder`, running `command` in it when
/// there is one.
///
/// Which terminals are installed is passed in rather than looked up, so the
/// order can be tested without installing anything.
fn command_for(
    folder: &str,
    asked_for: Option<&str>,
    installed: &dyn Fn(&str) -> bool,
    command: &[String],
) -> Option<Vec<String>> {
    let (program, known) = match asked_for.filter(|name| !name.is_empty() && installed(name)) {
        // A terminal named in the environment is the one to start, and it is
        // started the way the table starts it whenever the table knows it.
        // Falling through to the plain form for a name we have an entry for is
        // how a terminal opens in the wrong folder.
        Some(name) => (name, entry_for(name)),
        None => {
            let found = TERMINALS.iter().find(|entry| installed(entry.program))?;
            (found.program, Some(found))
        }
    };

    let mut argv = vec![program.to_string()];
    if let Some(flag) = known.and_then(|entry| entry.folder) {
        argv.push(flag.to_string());
        argv.push(folder.to_string());
    }
    if !command.is_empty() {
        // A terminal we know nothing else about is asked to run something the
        // way everything descended from xterm is asked.
        argv.push(known.map_or("-e", |entry| entry.command).to_string());
        argv.extend(command.iter().cloned());
    }
    Some(argv)
}

/// The table's entry for a terminal named either by its name or by its path,
/// since both are things people put in `$TERMINAL`.
fn entry_for(name: &str) -> Option<&'static Terminal> {
    let name = name.rsplit('/').next().unwrap_or(name);
    TERMINALS.iter().find(|entry| entry.program == name)
}

/// The command that opens the file in a tool, carrying what Comma has already
/// worked out about it.
///
/// This is the argument for naming the tools one by one rather than offering a
/// general way to open the file in something. A general one could pass a name
/// and a file, and the tool would be left to work the delimiter out again from
/// the bytes — while Comma is holding the answer, and for xled and xql holding
/// it in the spelling their own flags take.
fn invocation(tool: Tool, dialect: Dialect, header: bool, path: &str) -> Vec<String> {
    let mut argv = vec![tool.program().to_string()];
    match tool {
        Tool::Xled => argv.extend(reading_flags(dialect, header)),
        Tool::Xql => {
            // The backend is named rather than left to the extension, which a
            // file called .dsv has nothing xql would recognise in.
            argv.push("csv".to_string());
            argv.extend(reading_flags(dialect, header));
        }
        Tool::Nved => argv.extend(nved_specs(dialect, header)),
    }
    argv.push(path.to_string());
    argv
}

/// How xled and xql are told what Comma is reading. They take the same flags,
/// which is not a coincidence: they are the same family.
fn reading_flags(dialect: Dialect, header: bool) -> Vec<String> {
    let delimiter = match dialect.delimiter() {
        // The one delimiter that cannot be written down as itself, and both
        // tools spell it this way.
        '\t' => "\\t".to_string(),
        other => other.to_string(),
    };

    let mut flags = vec!["-d".to_string(), delimiter];
    if !header {
        flags.push("--no-header".to_string());
    }
    flags
}

/// The startup commands that open nved on the same view of the file Comma has.
///
/// nved runs several in the order they are given, which is what lets the mode
/// go down first and the header answer come after it: its presets carry a
/// header setting of their own, and saying it afterwards is what makes Comma's
/// answer the one that stands.
fn nved_specs(dialect: Dialect, header: bool) -> Vec<String> {
    let mut specs = Vec::new();
    if dialect == Dialect::comma() {
        specs.push("+csv".to_string());
    } else if dialect == Dialect::tab() {
        specs.push("+tsv".to_string());
    } else if dialect == Dialect::unit_separator() {
        specs.push("+asv".to_string());
    } else {
        // A preset sets the quoting along with the delimiter. A delimiter given
        // on its own does not, and nved starts with quoting off.
        specs.push(format!("+dsv {}", dialect.delimiter()));
        specs.push("+quotes on".to_string());
    }

    specs.push(match header {
        true => "+headers on".to_string(),
        false => "+headers off".to_string(),
    });
    specs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nothing(_: &str) -> bool {
        false
    }

    fn everything(_: &str) -> bool {
        true
    }

    fn only(installed: &'static str) -> impl Fn(&str) -> bool {
        move |name| name == installed
    }

    fn terminal(asked_for: Option<&str>, installed: &dyn Fn(&str) -> bool) -> Vec<String> {
        command_for("/tmp/here", asked_for, installed, &[])
            .expect("a terminal was installed for this test")
    }

    #[test]
    fn the_first_terminal_installed_is_the_one_opened() {
        assert_eq!(terminal(None, &everything), ["ptyxis", "-d", "/tmp/here"]);
        assert_eq!(
            terminal(None, &only("gnome-terminal")),
            ["gnome-terminal", "--working-directory", "/tmp/here"]
        );
    }

    #[test]
    fn a_terminal_with_no_flag_for_it_is_started_in_the_folder_instead() {
        assert_eq!(terminal(None, &only("xterm")), ["xterm"]);
    }

    #[test]
    fn nothing_installed_opens_nothing() {
        assert_eq!(command_for("/tmp/here", None, &nothing, &[]), None);
    }

    #[test]
    fn the_terminal_asked_for_is_the_one_started() {
        assert_eq!(terminal(Some("xterm"), &everything), ["xterm"]);
        // Asked for and not installed is not an answer, so the search goes on.
        assert_eq!(
            terminal(Some("xterm"), &only("gnome-terminal")),
            ["gnome-terminal", "--working-directory", "/tmp/here"]
        );
    }

    #[test]
    fn a_terminal_asked_for_by_name_still_gets_its_own_flag() {
        // The point of the table: a terminal we know is told which folder to
        // open in however it was chosen.
        assert_eq!(
            terminal(Some("gnome-terminal"), &everything),
            ["gnome-terminal", "--working-directory", "/tmp/here"]
        );
        assert_eq!(
            terminal(Some("/usr/bin/gnome-terminal"), &everything),
            [
                "/usr/bin/gnome-terminal",
                "--working-directory",
                "/tmp/here"
            ]
        );
    }

    #[test]
    fn a_terminal_nobody_knows_is_started_in_the_folder() {
        assert_eq!(terminal(Some("someterm"), &everything), ["someterm"]);
    }

    #[test]
    fn a_command_follows_whichever_flag_the_terminal_takes() {
        let command = vec!["xled".to_string(), "data.csv".to_string()];
        assert_eq!(
            command_for("/tmp/here", None, &only("gnome-terminal"), &command).unwrap(),
            [
                "gnome-terminal",
                "--working-directory",
                "/tmp/here",
                "--",
                "xled",
                "data.csv"
            ]
        );
        assert_eq!(
            command_for("/tmp/here", None, &only("xterm"), &command).unwrap(),
            ["xterm", "-e", "xled", "data.csv"]
        );
        // A terminal with no entry is asked the way xterm is asked.
        assert_eq!(
            command_for("/tmp/here", Some("someterm"), &everything, &command).unwrap(),
            ["someterm", "-e", "xled", "data.csv"]
        );
    }

    #[test]
    fn a_tool_is_told_the_delimiter_comma_is_reading_with() {
        assert_eq!(
            invocation(Tool::Xled, Dialect::semicolon(), true, "/tmp/data.dsv"),
            ["xled", "-d", ";", "/tmp/data.dsv"]
        );
        // The tab is the one that cannot be passed as itself.
        assert_eq!(
            invocation(Tool::Xled, Dialect::tab(), true, "/tmp/data.tsv"),
            ["xled", "-d", "\\t", "/tmp/data.tsv"]
        );
    }

    #[test]
    fn a_tool_is_told_whether_the_first_row_is_a_header() {
        assert_eq!(
            invocation(Tool::Xled, Dialect::comma(), false, "/tmp/data.csv"),
            ["xled", "-d", ",", "--no-header", "/tmp/data.csv"]
        );
    }

    #[test]
    fn xql_is_told_which_backend_to_use() {
        assert_eq!(
            invocation(Tool::Xql, Dialect::pipe(), true, "/tmp/data.dsv"),
            ["xql", "csv", "-d", "|", "/tmp/data.dsv"]
        );
    }

    #[test]
    fn nved_opens_in_the_view_that_matches_the_file() {
        assert_eq!(
            invocation(Tool::Nved, Dialect::comma(), true, "/tmp/data.csv"),
            ["nved", "+csv", "+headers on", "/tmp/data.csv"]
        );
        assert_eq!(
            invocation(Tool::Nved, Dialect::tab(), true, "/tmp/data.tsv"),
            ["nved", "+tsv", "+headers on", "/tmp/data.tsv"]
        );
        assert_eq!(
            invocation(Tool::Nved, Dialect::unit_separator(), true, "/tmp/data.asv"),
            ["nved", "+asv", "+headers on", "/tmp/data.asv"]
        );
    }

    #[test]
    fn nved_is_told_the_quoting_when_it_is_given_a_bare_delimiter() {
        assert_eq!(
            invocation(Tool::Nved, Dialect::semicolon(), false, "/tmp/data.dsv"),
            [
                "nved",
                "+dsv ;",
                "+quotes on",
                "+headers off",
                "/tmp/data.dsv"
            ]
        );
    }

    #[test]
    fn only_nved_is_offered_a_file_that_ends_its_records_with_a_separator() {
        assert!(Tool::Nved.reads(Dialect::unit_separator()));
        assert!(!Tool::Xled.reads(Dialect::unit_separator()));
        assert!(!Tool::Xql.reads(Dialect::unit_separator()));
        for tool in Tool::ALL {
            assert!(tool.reads(Dialect::comma()));
        }
    }
}
