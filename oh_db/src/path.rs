// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use std::error::Error;
use std::fmt;

make_error!(PathError; {
    NonAbsolutePath => String,
    Dotfile => String,
    EmptyComponent => String,
    InvalidCharacter => String,
    InvalidWhitespace => String,
    UnreachablePattern => String,
    NoParent => String,
    NoBasename => String
});
pub type PathResult<T> = Result<T, PathError>;

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
    pub fn new(raw: &str) -> PathResult<PathBuilder> {
        if !raw.starts_with('/') {
            return Err(PathError::NonAbsolutePath(raw.to_owned()));
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
        for part in raw.split('/').skip(1) {
            if try!(PathBuilder::validate_path_or_glob_component(part)) {
                contains_glob_chars = true;
            }
            parts.push(part.to_owned());
        }
        return Ok(PathBuilder {
           parts: parts,
           contains_glob_chars: contains_glob_chars
        });
    }

    // Returns whether there are glob chars in the component.
    fn validate_path_or_glob_component(part: &str) -> PathResult<bool> {
        let mut contains_glob_chars = false;
        if part.len() == 0 {
            return Err(PathError::EmptyComponent("".to_owned()));
        }
        if part.starts_with(".") {
            return Err(PathError::Dotfile(part.to_owned()));
        }
        for c in part.chars() {
            if c == '\\' ||
               c == '/' ||
               c == ':' ||
               c == ','
            {
                return Err(PathError::InvalidCharacter(
                           part.to_owned() + " character: " + &c.to_string()));
            }
            if c.is_whitespace() {
                return Err(PathError::InvalidWhitespace(part.to_owned()));
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
        return Ok(contains_glob_chars);
    }

    /// Check that the given string is a valid path component. Returns an
    /// error if the part contains invalid characters, including glob characters.
    pub fn validate_path_component(part: &str) -> PathResult<()> {
        if try!(PathBuilder::validate_path_or_glob_component(part)) {
            return Err(PathError::InvalidCharacter(part.to_owned()));
        }
        return Ok(());
    }

    /// Return the built path, if it is a path and not a glob. Otherwise returns
    /// an error.
    pub fn finish_path(self) -> PathResult<Path> {
        if self.contains_glob_chars {
            return Err(PathError::InvalidCharacter(
                       format!("unexpected glob character")));
        }
        return Ok(Path {parts: self.parts});
    }

    /// Return the built glob.
    pub fn finish_glob(self) -> PathResult<Glob> {
        // Construct Glob part matchers from our strings.
        let mut parts = Vec::new();
        for part in self.parts {
            parts.push(try!(GlobComponent::new(&part)));
        }

        // Check that we do not have multiple RecursiveSequence in a row.
        // e.g. /foo/**/**/bar does not make any sense.
        {
            let i = parts.iter();
            let j = parts.iter().skip(1);
            for (a, b) in i.zip(j) {
                if a.tokens[0] == GlobToken::AnyRecursiveSequence &&
                   b.tokens[0] == GlobToken::AnyRecursiveSequence
                {
                    return Err(PathError::UnreachablePattern("**".to_owned()));
                }
            }
        }

        return Ok(Glob {
            parts: parts,
            is_exact: !self.contains_glob_chars
        });
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

    pub fn parent(&self) -> PathResult<Path> {
        if self.parts.len() == 0 {
            return Err(PathError::NoParent("already at top".to_owned()));
        }
        let mut parent_parts: Vec<String> = Vec::new();
        for part in self.parts.iter().take(self.parts.len() - 1) {
            parent_parts.push(part.clone());
        }
        return Ok(Path {parts: parent_parts});
    }

    pub fn basename(&self) -> PathResult<String> {
        match self.parts.last() {
            None => Err(PathError::NoBasename("the root has no name".to_owned())),
            Some(p) => Ok(p.clone())
        }
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
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Glob {
    parts: Vec<GlobComponent>,
    is_exact: bool
}

impl Glob {
    pub fn iter(&self) -> GlobIter {
        GlobIter { parts: &self.parts, offset: 0 }
    }

    fn to_str(&self) -> String {
        "/".to_owned() + &self.parts.iter()
                              .map(|x| x.source.clone())
                              .collect::<Vec<_>>()
                              .join("/")
    }
}

impl fmt::Display for Glob {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
enum GlobToken {
    Char(char),
    AnyChar, // ?
    AnySequence, // *
    AnyRecursiveSequence, // **

    // TODO: decide whether we want to bother supporting these.
    //AnyWithin(Vec<char>), // []
    //AnyExcept(Vec<char>) // [!]
}

// A single path component of a glob.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct GlobComponent {
    source: String,
    tokens: Vec<GlobToken>
}

impl GlobComponent {
    fn new(part: &str) -> PathResult<GlobComponent> {
        // ** Can only be a whole path component, so we can check for it up front.
        let mut tokens = Vec::new();
        if part == "**" {
            tokens.push(GlobToken::AnyRecursiveSequence);
            return Ok(GlobComponent { source: part.to_owned(), tokens: tokens });
        }

        let mut i = 0;
        let chars = part.chars().collect::<Vec<_>>();
        while i < chars.len() {
            match chars[i] {
                '?' => {
                    tokens.push(GlobToken::AnyChar);
                    i += 1;
                }
                '*' => {
                    // Detect ** not as whole part or, ***, ****, etc.
                    if chars.len() > i + 1 && chars[i + 1] == '*' {
                        return Err(PathError::InvalidCharacter("***+".to_owned()));
                    }
                    tokens.push(GlobToken::AnySequence);
                    i += 1;
                }
                c => {
                    tokens.push(GlobToken::Char(c));
                    i += 1;
                }
            }
        }
        return Ok(GlobComponent { source: part.to_owned(), tokens: tokens });
    }

    pub fn matches(&self, name: &str) -> bool {
        /*
        assert!(self.tokens.len() > 0);
        let chars = name.chars().collect::<Vec<_>>();
        let mut i = 0;
        for token in self.tokens {
            if i >= chars.len() {
                return false;
            }
            match token {
                GlobToken::Char(c) => {
                    if chars[i] != c {
                        return false;
                    }
                    i += 1;
                }
                _ => {}
            }
        }
        */
        return true;
    }
}

/// An iteration of the components of a Path.
#[derive(Debug)]
pub struct GlobIter<'a> {
    parts: &'a Vec<GlobComponent>,
    offset: usize
}

impl<'a> Iterator for GlobIter<'a> {
    type Item = &'a GlobComponent;
    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.parts.len() {
            return None;
        }

        let off = self.offset;
        self.offset += 1;
        return Some(&self.parts[off]);
    }
}

#[cfg(test)]
mod tests {
    extern crate env_logger;
    use super::*;

    fn make_path(p: &str) -> Path {
        PathBuilder::new(p).unwrap().finish_path().unwrap()
    }

    macro_rules! make_badpath_tests {
        ( [ $( ($expect:expr, $name:ident, $string:expr) ),* ] ) =>
        {
            $(
                #[test]
                #[should_panic(expected=$expect)]
                fn $name() {
                    make_path($string);
                }
            )*
        }
    }

    make_badpath_tests!([
        ("NonAbsolutePath", test_empty_path, ""),
        ("NonAbsolutePath", test_relative_path, "foo/bar"),
        ("EmptyComponent", test_empty_component_root, "//"),
        ("EmptyComponent", test_empty_component_front, "//foo"),
        ("EmptyComponent", test_empty_component_back, "/foo/"),
        ("EmptyComponent", test_empty_component_middle, "/foo//bar"),
        ("Dotfile", test_dotfile_self, "/foo/."),
        ("Dotfile", test_dotfile_self_middle, "/foo/./bar"),
        ("Dotfile", test_dotfile_parent, "/foo/.."),
        ("Dotfile", test_dotfile_parent_middle, "/foo/../bar"),
        ("Dotfile", test_dotfile_hidden, "/foo/.bar"),
        ("Dotfile", test_dotfile_hidden_middle, "/foo/.bar/baz"),
        ("InvalidWhitespace", test_whitespace_tab, "/foo/a\tb/baz"),
        ("InvalidWhitespace", test_whitespace_vertical_tab, "/foo/a\x0Bb/baz"),
        ("InvalidWhitespace", test_whitespace_newline, "/foo/a\nb/baz"),
        ("InvalidWhitespace", test_whitespace_carriage_return, "/foo/a\rb/baz"),
        ("InvalidWhitespace", test_whitespace_nbsp, "/foo/a\u{A0}b/baz"),
        ("InvalidCharacter", test_invalid_backslash, "/foo/a\\b/baz"),
        ("InvalidCharacter", test_invalid_colon, "/foo/a:b/baz"),
        ("InvalidCharacter", test_invalid_comma, "/foo/a,b/baz"),
        ("InvalidCharacter", test_invalid_star, "/foo/a*b/baz"),
        ("InvalidCharacter", test_invalid_question, "/foo/a?b/baz"),
        ("InvalidCharacter", test_invalid_open_bracket, "/foo/a[b/baz"),
        ("InvalidCharacter", test_invalid_close_bracket, "/foo/a]b/baz"),
        ("InvalidCharacter", test_invalid_exclamation, "/foo/a!b/baz")
    ]);

    fn make_glob(p: &str) -> Glob {
        PathBuilder::new(p).unwrap().finish_glob().unwrap()
    }

    macro_rules! make_badglob_tests {
        ( [ $( ($expect:expr, $name:ident, $string:expr) ),* ] ) =>
        {
            $(
                #[test]
                #[should_panic(expected=$expect)]
                fn $name() {
                    make_glob($string);
                }
            )*
        }
    }

    make_badglob_tests!([
        ("UnreachablePattern", test_unreachable_sole, "/**/**"),
        ("UnreachablePattern", test_unreachable_end, "/a/**/**"),
        ("UnreachablePattern", test_unreachable_prefix, "/**/**/b"),
        ("UnreachablePattern", test_unreachable_middle, "/a/**/**/b"),
        ("InvalidCharacter", test_invalid_multistar2_start, "/a/**foo/b"),
        ("InvalidCharacter", test_invalid_multistar2_end, "/a/foo**/b"),
        ("InvalidCharacter", test_invalid_multistar2_middle, "/a/fo**oo/b"),
        ("InvalidCharacter", test_invalid_multistar3, "/a/***/b"),
        ("InvalidCharacter", test_invalid_multistar4, "/a/****/b")
    ]);

    #[test]
    fn test_construct_glob() {
        PathBuilder::new("/?a/a?/a?b/*c/c*/c*d").unwrap().finish_glob().unwrap();
    }

    macro_rules! make_glob_match_tests {
        ( [ $(
           ($name: ident,
            $glob:expr,
            [ $( $successes:expr ),* ],
            [ $( $failures:expr ),* ])
        ),* ] ) =>
        {
            $(
                #[test]
                fn $name() {
                    let success: Vec<&'static str> = vec![ $($successes),* ];
                    let failure: Vec<&'static str> = vec![ $($failures),* ];
                    let glob = make_glob($glob);
                    let component = glob.iter().next().unwrap();
                    for part in success {
                        assert!(component.matches(part));
                    }
                    for part in failure {
                        assert!(!component.matches(part));
                    }
                }
            )*
        }
    }

    make_glob_match_tests!([
        (test_match_q_start, "/?a", ["/aa", "/ba", "/Xa"], ["/ab", "/Xaa", "/Xa/a"])
    ]);
    #[test]
    fn test_match_glob_component() {
        let glob = make_glob("/?a");
        let component = glob.iter().next().unwrap();
        assert!(component.matches("aa"));
        assert!(component.matches("ba"));
        assert!(component.matches("ca"));
    }
}
