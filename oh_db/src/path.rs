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
///     - any whitespace character
///     - any characters special to yaml
///       \ : ,
///     - any globbing characters:
///       ? * [ ] !
///
/// Globs are just like paths, except that they relax the last check and allow
/// glob characters. Both paths and globs can be constructed using a PathBuilder.
pub struct PathBuilder {
    parts: Vec<String>,
    contains_glob_chars: bool
}

impl PathBuilder {
    /// Parse the given raw UTF-8 string. This function may return an error
    /// if it cannot be a path or glob.
    pub fn new(raw: &str) -> TreeResult<PathBuilder> {
        if !raw.starts_with('/') {
            return Err(TreeError::NonAbsolutePath(raw.to_owned()));
        }

        // Split produces two empty strings for "/", so just handle it separately
        // instead of trying to do something smart in the loop below.
        if raw == "/" {
            return Ok(PathBuilder {
                          parts: Vec::new(),
                          contains_glob_chars: false
                      });
        }

        // Note that since we start with /, we have to skip the first, empty, part.
        let mut contains_glob_chars = false;
        let mut parts: Vec<String> = Vec::new();
        for (i, part) in raw.split('/').skip(1).enumerate() {
            if part.len() == 0 {
                return Err(TreeError::EmptyComponent(
                           raw.to_owned() + " at part " + &i.to_string()));
            }
            if part.starts_with(".") {
                return Err(TreeError::Dotfile(
                           raw.to_owned() + " at part " + &i.to_string()));
            }
            for c in part.chars() {
                if c == '\\' ||
                   c == '/' ||
                   c == ':' ||
                   c == ','
                {
                    return Err(TreeError::InvalidCharacter(
                        raw.to_owned() + " character: " + &c.to_string()));
                }
                // FIXME: whitespace should just be invalid character.
                if c.is_whitespace() {
                    return Err(TreeError::InvalidWhitespace(
                        format!("{} at 0x{:X}", raw, c as u32)));
                }
                if c == '?' ||
                   c == '*' ||
                   c == '[' ||
                   c == ']' ||
                   c == '!'
                {
                    contains_glob_chars = true;
                }
            }
            parts.push(part.to_owned());
        }
        return Ok(PathBuilder {
           parts: parts,
           contains_glob_chars: contains_glob_chars
        });
    }

    /// Return the given path, if it is a path and not a glob. Otherwise returns
    /// an error.
    pub fn finish_path(self) -> TreeResult<Path> {
        if self.contains_glob_chars {
            return Err(TreeError::InvalidCharacter(
                       format!("unexpected glob character")));
        }
        return Ok(Path {parts: self.parts});
    }
}

/// A path refers to a single location in the Tree. The location may or may
/// not exist, path is just a reference to a location.
#[derive(Debug, Clone)]
pub struct Path {
    parts: Vec<String>
}

impl Path {
    // Build a new String containing the canonical representation of this path.
    pub fn to_str(&self) -> String {
        return "/".to_owned() + &self.parts.join("/")
    }

    pub fn iter(&self) -> PathIter {
        PathIter { parts: &self.parts, offset: 0 }
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

/// An iteration of the components of a Path.
#[derive(Debug)]
pub struct PathIter<'a> {
    parts: &'a Vec<String>,
    offset: usize
}

impl<'a> Iterator for PathIter<'a> {
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

/// A glob refers to one or more locations in a Tree.
pub struct Glob {
    parts: Vec<String>,
    is_exact: bool
}

/// Verify that the given glob obeys the same restrictions as those set on tree
/// paths.
pub fn validate_glob(glob: &Pattern) -> TreeResult<()> {
    validate_path_common(glob.as_str(), true)
}

/// Attempt to create a path from the given glob. If the path creation fails,
/// e.g. because we contain glob characters, we return None, otherwise we
/// return the new Path. Note that this does not validate the glob: that
/// must still be done before evaluating it.
pub fn maybe_become_path(glob: &Pattern) -> Option<Path> {
    match PathBuilder::new(glob.as_str()) {
        Ok(p) => match p.finish_path() {
            Ok(p) => Some(p),
            Err(_) => None
        },
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

