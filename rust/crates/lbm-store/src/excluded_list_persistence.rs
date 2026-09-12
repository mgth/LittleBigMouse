//! The excluded-processes file — port of `Persistence/ExcludedListPersistence.cs`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::excluded_process_defaults::{contains_entry, ALL, HEADER, LEGACY_V0, VERSION};
use crate::json_format::decode_text;
use crate::layout_dtos::GlobalOptionsDto;
use crate::layout_store::LayoutStore;

/// C# `Environment.NewLine`, which `File.WriteAllLines` ends every line with.
#[cfg(windows)]
const NEW_LINE: &str = "\r\n";
/// C# `Environment.NewLine`, which `File.WriteAllLines` ends every line with.
#[cfg(not(windows))]
const NEW_LINE: &str = "\n";

/// C# `ExcludedListPersistence`: the excluded-processes list — its file, its default
/// entries and the one-time top-up that brings new defaults to existing
/// installations. It sits beside [`LayoutStore`] rather than in it, because the list
/// is a plain-text FILE the user may edit by hand (`Excluded.txt`: one entry per line,
/// `:` lines are comments, empty lines are nothing); the store only holds the version
/// counter that keeps the top-up one-time. The daemon no longer reads it — the agent
/// does, and hands the daemon the list (`Command::Excluded`).
///
/// The writes a load makes are best effort, as in C#: a read-only or missing directory
/// must never keep the app from starting, and the in-memory list is correct either
/// way. The C# class holds the store; here it is passed to [`load`](Self::load), the
/// only method that writes through it, so the engine can own both.
pub struct ExcludedListPersistence<F> {
    excluded_list_file: F,
    applied_defaults_version: Option<i32>,
    comments: Vec<String>,
}

impl<F: Fn() -> PathBuf> ExcludedListPersistence<F> {
    /// C# `ExcludedListPersistence(store, excludedListFile)`. `excluded_list_file`
    /// gives the file's path, asked again at every load and write.
    pub fn new(excluded_list_file: F) -> Self {
        ExcludedListPersistence {
            excluded_list_file,
            applied_defaults_version: None,
            comments: Vec::new(),
        }
    }

    /// C# `ExcludedListPersistence.AppliedDefaultsVersion`: the top-up version already
    /// applied, read from the store at load time and to be round-tripped into every
    /// global-options write (it is not part of the options model) so the one-time
    /// migration stays one-time.
    pub fn applied_defaults_version(&self) -> Option<i32> {
        self.applied_defaults_version
    }

    /// C# `ExcludedListPersistence.Load`: fill `list` from the file, seeding or topping
    /// up the defaults as needed.
    ///
    /// `global` is the options document as READ from the store: it carries the applied
    /// version in, and the top-up records the new version in it and writes it back
    /// through `store` — the stored document rather than a model-derived one, so a
    /// migration running during a load cannot persist half-initialized options.
    ///
    /// The `:` lines are kept aside and re-emitted on top at every write: the daemon
    /// skips them, so they are not exclusions and have no business in the list the user
    /// edits — but they may be the user's own annotations, which must stay on disk.
    ///
    /// Fails only when the file exists and cannot be read (C# `File.ReadAllLines`
    /// throws out of the load); `list` is then empty.
    pub fn load<S: LayoutStore + ?Sized>(
        &mut self,
        store: &S,
        list: &mut Vec<String>,
        global: Option<&mut GlobalOptionsDto>,
    ) -> io::Result<()> {
        self.applied_defaults_version = global.as_deref().and_then(|g| g.excluded_defaults_version);

        list.clear();

        let file = (self.excluded_list_file)();
        if !file.is_file() {
            // First run: seed the defaults and write the file the daemon reads, with
            // the same content as the UI's CreateExcludedFile, header included, since
            // either can be the one to create it. The version is remembered so the next
            // global-options write records the top-up as applied.
            self.comments = vec![HEADER.to_owned()];
            list.extend(ALL.iter().map(|entry| (*entry).to_owned()));
            self.applied_defaults_version = Some(VERSION);
            let _ = self.write_to(&file, list.iter());
            return Ok(());
        }

        // An empty line is nothing, a ':' line is a comment, everything else is an
        // exclusion — the split the daemon used to make for itself, and which this is
        // now the only implementation of: what comes out of here is what the daemon is
        // handed, so the list the user edits and the list that filters are one list.
        let bytes = fs::read(&file)?;
        let mut comments = Vec::new();
        for line in read_lines(&decode_text(&bytes)) {
            if line.starts_with(':') {
                comments.push(line.to_owned());
                continue;
            }
            if line.is_empty() {
                continue;
            }
            list.push(line.to_owned());
        }
        self.comments = comments;

        self.migrate_defaults(store, list, global, &file);
        Ok(())
    }

    /// C# `ExcludedListPersistence.Write`: rewrite the file the daemon reads, comments
    /// first. Best effort in C#, which swallows the failure; reported here.
    pub fn write<I>(&self, entries: I) -> io::Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.write_to(&(self.excluded_list_file)(), entries)
    }

    /// C# `ExcludedListPersistence.Write(file, entries)`: `File.WriteAllLines` of the
    /// comments then the entries — UTF-8 without a byte-order mark, every line ended
    /// by the platform's new line.
    fn write_to<I>(&self, file: &Path, entries: I) -> io::Result<()>
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        let mut text = String::new();
        for comment in &self.comments {
            text.push_str(comment);
            text.push_str(NEW_LINE);
        }
        for entry in entries {
            text.push_str(entry.as_ref());
            text.push_str(NEW_LINE);
        }
        fs::write(file, text)
    }

    /// C# `ExcludedListPersistence.MigrateDefaults`: the one-time top-up of the default
    /// exclusion list. When new default entries ship (e.g. Xbox game folders, #494)
    /// they must reach users who already have an `Excluded.txt` — a fresh seed only
    /// covers new installs. Runs once per [`VERSION`] (tracked in the store) and only
    /// when the list still holds every previous default, so a customized list — or a
    /// default the user deliberately removed later — is left untouched. Rewrites the
    /// file too, since the daemon reads it, not the in-memory list.
    fn migrate_defaults<S: LayoutStore + ?Sized>(
        &mut self,
        store: &S,
        list: &mut Vec<String>,
        global: Option<&mut GlobalOptionsDto>,
        file: &Path,
    ) {
        if self.applied_defaults_version.unwrap_or(0) >= VERSION {
            return;
        }

        // Only top up a list that still holds all the previous defaults (the user kept
        // them). Separator-insensitive: pre-V2 Linux lists were seeded with the
        // Windows-style entries.
        if LEGACY_V0
            .iter()
            .all(|entry| contains_entry(list.iter(), entry))
        {
            let mut added = false;
            for entry in ALL {
                if contains_entry(list.iter(), entry) {
                    continue;
                }
                list.push((*entry).to_owned());
                added = true;
            }

            if added {
                let _ = self.write_to(file, list.iter());
            }
        }

        self.applied_defaults_version = Some(VERSION);
        let mut fresh = GlobalOptionsDto::default();
        let dto = global.unwrap_or(&mut fresh);
        dto.excluded_defaults_version = self.applied_defaults_version;
        let _ = store.write_global_options(dto);
    }
}

/// The lines of a text as C# `File.ReadAllLines` returns them (`StreamReader.ReadLine`):
/// a line ends at `\r\n`, `\n` or a lone `\r`, and a final line ending does not start
/// an empty line.
///
/// Kept faithful to C# because the file is shared with it and with the user's editor.
/// (The daemon used to split it a second time, with Rust's `str::lines`, which differs
/// on a lone `\r` and on a byte-order mark; it is handed the parsed list now, so there
/// is one reader left.)
fn read_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find(['\r', '\n']) {
            Some(end) => {
                lines.push(&rest[..end]);
                let terminator = if rest[end..].starts_with("\r\n") {
                    2
                } else {
                    1
                };
                rest = &rest[end + terminator..];
            }
            None => {
                lines.push(rest);
                break;
            }
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_end_like_dotnet_readline() {
        assert_eq!(read_lines(""), Vec::<&str>::new());
        assert_eq!(read_lines("a"), ["a"]);
        assert_eq!(read_lines("a\n"), ["a"]);
        assert_eq!(read_lines("\n"), [""]);
        assert_eq!(read_lines("a\n\nb"), ["a", "", "b"]);
        assert_eq!(read_lines("a\r\nb\rc\n"), ["a", "b", "c"]);
        assert_eq!(read_lines("a\r\r\n"), ["a", ""]);
        assert_eq!(read_lines(" \t "), [" \t "]);
    }
}
