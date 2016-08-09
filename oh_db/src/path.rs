// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use glob::Pattern;
use tree::{TreeError, TreeResult};
use std::fmt;

/// OpenHouse paths have somewhat stricter rules than a typical filesystem. The
/// rules are:
///   * must be unix style
///   * must be absolute
///   * path components may not start with '.'
///   * path components must not be empty, e.g. //
///   * must only contain printable UTF-8 characters
///   * the following characters are disallowed:
///     - any whitespace character other than 0x20 (plain ol space)
///     - any characters special to yaml:
///       \ : ,
///     - any globbing characters:
///       ? * [ ] !
#[derive(Debug, Clone)]
pub struct TreePath {
    raw: String,
    parts: Vec<String>
}

impl TreePath {
    /// Validate and create a new Tree path.
    pub fn new(raw_path: &str) -> TreeResult<TreePath> {
        try!(validate_path_common(raw_path, false));

        let mut path = TreePath {
            raw: raw_path.to_owned(),
            parts: Vec::new()
        };
        if path.raw != "/" {
            for part in path.raw.split('/').skip(1) {
                path.parts.push(part.to_owned());
            }
        }

        assert_eq!(path.raw, "/".to_owned() + &path.parts.join("/"));
        return Ok(path);
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    pub fn iter(&self) -> TreePathIter {
        TreePathIter { parts: &self.parts, offset: 0 }
    }
}

impl fmt::Display for TreePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.raw)
    }
}

#[derive(Debug)]
pub struct TreePathIter<'a> {
    parts: &'a Vec<String>,
    offset: usize
}

impl<'a> Iterator for TreePathIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.parts.len() {
            return None;
        }

        let off = self.offset;
        self.offset += 1;
        return Some(&self.parts[off]);
    }
}

/// Verify that the given glob obeys the same restrictions as those set on tree
/// paths.
pub fn validate_glob(glob: &Pattern) -> TreeResult<()> {
    validate_path_common(glob.as_str(), true)
}

/// Attempt to create a path from the given glob. If the path creation fails,
/// e.g. because we contain glob characters, we return None, otherwise we
/// return the new TreePath. Note that this does not validate the glob: that
/// must still be done before evaluating it.
pub fn maybe_become_path(glob: &Pattern) -> Option<TreePath> {
    match TreePath::new(glob.as_str()) {
        Ok(p) => Some(p),
        Err(_) => None
    }
}

fn validate_path_common(path: &str, allow_glob: bool) -> TreeResult<()>
{
    if !path.starts_with('/') {
        return Err(TreeError::NonAbsolutePath(path.to_owned()));
    }

    // Split produces two empty strings for "/", so just handle it separately
    // instead of trying to do something smart in the loop below.
    if path == "/" {
        return Ok(());
    }

    // Note that since we start with /, we have to skip the first, empty, part.
    for (i, part) in path.split('/').skip(1).enumerate() {
        try!(validate_path_component(path, i, part, allow_glob));
    }
    return Ok(());
}

pub fn validate_path_component(path: &str, i: usize, part: &str, allow_glob: bool)
    -> TreeResult<()>
{
    if part.len() == 0 {
        return Err(TreeError::EmptyComponent(
                path.to_owned() + " at part " + &i.to_string()));
    }
    if part.starts_with(".") {
        return Err(TreeError::Dotfile(
                path.to_owned() + " at part " + &i.to_string()));
    }

    for c in part.chars() {
        if c == '\\' ||
           c == '/' ||
           c == ':' ||
           c == ',' ||
           (!allow_glob && c == '?') ||
           (!allow_glob && c == '*') ||
           (!allow_glob && c == '[') ||
           (!allow_glob && c == ']') ||
           (!allow_glob && c == '!')
        {
            return Err(TreeError::InvalidCharacter(
                path.to_owned() + " character: " + &c.to_string()));
        }
        if c.is_whitespace() && c != ' ' {
            return Err(TreeError::InvalidWhitespace(
                format!("{} at 0x{:X}", path, c as u32)));
        }
    }
    return Ok(());
}

