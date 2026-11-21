//! What a dry run says will happen (R-SSH-56): rsync's `--itemize-changes` lines, `rsync(1)`'s
//! "--itemize-changes" section: `YXcstpoguax path`, where Y is the update (`<` sent, `>` received, `c` created,
//! `h` hard link, `.` attributes only), X the file type, and a new item has `+` in every other place;
//! `*deleting   path` for what `--delete` removes.

/// The most names kept of each kind; every one is counted.
pub const SHOWN: usize = 200;

/// Items of one kind: how many, and the first [`SHOWN`] names.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Listed {
    pub count: usize,
    pub names: Vec<String>,
}

impl Listed {
    fn add(&mut self, name: &str) {
        self.count += 1;
        if self.names.len() < SHOWN {
            self.names.push(name.to_owned());
        }
    }
}

/// What will be added, changed and deleted at the target, read as the dry run's output comes in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Changes {
    pub added: Listed,
    pub changed: Listed,
    pub deleted: Listed,
    /// The start of a line not yet ended, as bytes: a read may end inside a character.
    partial: Vec<u8>,
}

impl Changes {
    /// Takes in more of the output.
    pub fn push(&mut self, data: &[u8]) {
        self.partial.extend_from_slice(data);
        while let Some(end) = self.partial.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = self.partial.drain(..=end).collect();
            self.line(String::from_utf8_lossy(&line).trim_end_matches(['\n', '\r']));
        }
    }

    /// The output has ended: reads a last line without a line end.
    pub fn finish(&mut self) {
        let line = std::mem::take(&mut self.partial);
        self.line(String::from_utf8_lossy(&line).trim_end_matches('\r'));
    }

    /// Nothing will happen: the target is up to date.
    pub fn is_empty(&self) -> bool {
        self.added.count + self.changed.count + self.deleted.count == 0
    }

    fn line(&mut self, line: &str) {
        if let Some(name) = line.strip_prefix("*deleting") {
            let name = name.trim_start();
            if !name.is_empty() {
                self.deleted.add(name);
            }
            return;
        }
        let Some((flags, name)) = line.split_once(' ') else {
            return;
        };
        let mut chars = flags.chars();
        let (Some(update), Some(kind)) = (chars.next(), chars.next()) else {
            return;
        };
        if !"<>ch.".contains(update) || !"fdLDS".contains(kind) || name.is_empty() {
            // Not an itemized line: a warning, or a message of the server's.
            return;
        }
        let rest = chars.as_str();
        if !rest.is_empty() && rest.chars().all(|c| c == '+') {
            self.added.add(name);
        } else if update == '.' && kind == 'd' {
            // A folder whose times or permissions only will be set: not worth listing.
        } else {
            self.changed.add(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(output: &[&[u8]]) -> Changes {
        let mut changes = Changes::default();
        for part in output {
            changes.push(part);
        }
        changes.finish();
        changes
    }

    #[test]
    fn a_dry_run_is_read_as_added_changed_and_deleted_r_ssh_56() {
        let changes = read(&[
            "cd+++++++++ rapporter æøå/\n<f+++++++++ rapporter æøå/januar.pdf\r\n".as_bytes(),
            b".d..t...... ./\n<f.st...... priser.csv\n.f...p..... script.sh\n",
            "cL+++++++++ seneste -> rapporter æøå\n*deleting   gammel fil.txt\n".as_bytes(),
            b"sending incremental file list\nskipping non-regular file \"x\"\n<f+++++",
            b"++++ sidste.txt",
        ]);
        assert_eq!(
            changes.added.names,
            [
                "rapporter æøå/",
                "rapporter æøå/januar.pdf",
                "seneste -> rapporter æøå",
                "sidste.txt"
            ],
            "a line split between two reads is read whole"
        );
        assert_eq!(changes.changed.names, ["priser.csv", "script.sh"]);
        assert_eq!(changes.deleted.names, ["gammel fil.txt"]);
        assert!(!changes.is_empty());
    }

    #[test]
    fn an_up_to_date_target_has_no_changes_and_long_lists_are_counted_r_ssh_56() {
        assert!(read(&[b"", b".d..t...... ./\n"]).is_empty());
        let split = "<f+++++++++ æ.txt\n".as_bytes();
        let changes = read(&[&split[..13], &split[13..]]);
        assert_eq!(
            changes.added.names,
            ["æ.txt"],
            "a character split between reads"
        );
        let many = (0..SHOWN + 5)
            .map(|i| format!(">f+++++++++ f{i}\n"))
            .collect::<Vec<_>>()
            .concat();
        let changes = read(&[many.as_bytes()]);
        assert_eq!(changes.added.count, SHOWN + 5);
        assert_eq!(changes.added.names.len(), SHOWN);
    }
}
