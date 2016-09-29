// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use std::error::Error;
use std::fmt;
use std::str;

make_error_system!(
    PathErrorKind => PathError => PathResult {
        Dotfile,
        EmptyComponent,
        InvalidCharacter,
        InvalidControlCharacter,
        InvalidGlobCharacter,
        InvalidWhitespaceCharacter,
        MismatchedBraces,
        NonAbsolutePath,
        UnreachablePattern
    });


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
///       ? * [ ] { } !
///
/// Globs are just like paths, except that they relax the last check and allow
/// glob characters. Both paths and globs can be constructed using a PathBuilder.
pub struct PathBuilder {
    parts: Vec<String>,
    contains_glob_chars: bool
}

fn is_invalid_character(c: char) -> bool {
    c == '!' || // Reserved for glob usage
    c == '[' || // "
    c == ']' || // "
    c == '/' || // Should be removed already by .split('/')
    c == '\\'|| // Confusing and restricted in windows paths regardless.
    c == ':' || // "
    c == '"' || // "
    c == '\''   // "
}

fn is_glob_character(c: char) -> bool {
    c == '?' ||
    c == '*' ||
    c == '{' ||
    c == '}' ||
    c == ','
}

impl PathBuilder {
    /// Parse the given raw UTF-8 string. This function may return an error
    /// if it cannot be a path or glob.
    pub fn new(raw: &str) -> PathResult<PathBuilder> {
        if !raw.starts_with('/') {
            return Err(PathError::NonAbsolutePath(raw));
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
            return Err(PathError::EmptyComponent(""));
        }
        if part.starts_with(".") {
            return Err(PathError::Dotfile(part));
        }
        for c in part.chars() {
            if is_invalid_character(c) {
                return Err(PathError::InvalidCharacter(
                           &(part.to_owned() + " character: " + &c.to_string())));
            }
            if c.is_control() {
                return Err(PathError::InvalidControlCharacter(part));
            }
            if c.is_whitespace() {
                return Err(PathError::InvalidWhitespaceCharacter(part));
            }

            if is_glob_character(c) {
                contains_glob_chars = true;
            }
        }
        return Ok(contains_glob_chars);
    }

    /// Check that the given string is a valid path component. Returns an
    /// error if the part contains invalid characters, including glob characters.
    pub fn validate_path_component(part: &str) -> PathResult<()> {
        if try!(PathBuilder::validate_path_or_glob_component(part)) {
            return Err(PathError::InvalidGlobCharacter(part));
        }
        return Ok(());
    }

    /// Return the built path, if it is a path and not a glob. Otherwise returns
    /// an error.
    pub fn finish_path(self) -> PathResult<Path> {
        if self.contains_glob_chars {
            return Err(PathError::InvalidGlobCharacter("unexpected glob character"));
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
                    return Err(PathError::UnreachablePattern("**"));
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
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
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

    #[cfg(test)]
    pub fn parent(&self) -> PathResult<Path> {
        if self.parts.len() == 0 {
            return Err(PathError::EmptyComponent("already at top"));
        }
        let mut parent_parts: Vec<String> = Vec::new();
        for part in self.parts.iter().take(self.parts.len() - 1) {
            parent_parts.push(part.clone());
        }
        return Ok(Path {parts: parent_parts});
    }

    #[cfg(test)]
    pub fn basename(&self) -> PathResult<String> {
        match self.parts.last() {
            None => Err(PathError::EmptyComponent("the root has no name")),
            Some(p) => Ok(p.clone())
        }
    }

    pub fn slash(&self, part: &str) -> PathResult<Path> {
        // FIXME: take &PathComponent so that we don't lose verification.
        let mut out = self.parts.clone();
        out.push(part.to_owned());
        return Ok(Path {parts: out});
    }

    pub fn root() -> Path {
        return Path { parts: vec![] };
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
    /// Produce the components of a glob, one at a time.
    pub fn iter(&self) -> GlobIter {
        GlobIter { parts: &self.parts, offset: 0 }
    }

    fn to_str(&self) -> String {
        "/".to_owned() + &self.parts.iter()
                              .map(|x| x.source.clone())
                              .collect::<Vec<_>>()
                              .join("/")
    }

    /// Check if the given path matches this glob.
    pub fn matches(&self, path: &Path) -> bool {
        let mut path_parts = path.iter();
        let mut glob_parts = self.iter();
        loop {
            let glob_part = match glob_parts.next() {
                Some(p) => p,
                // Exit to check if our path is exhausted too.
                None => break
            };
            let mut path_part = match path_parts.next() {
                Some(p) => p,
                // If there are more glob parts, we might still match if the
                // glob matcher is ** and it is the last part.
                None => {
                    return glob_part.source == "**" &&
                           glob_parts.next() == None;
                }
            };
            match glob_part.matches(path_part) {
                MatchResult::Match => continue,
                MatchResult::NoMatch => return false,
                MatchResult::MatchRecurse => {
                    // Fast-forward until this path_part or a subsequent one
                    // matches the *next* glob_part.
                    let glob_next = match glob_parts.next() {
                        Some(g) => g,
                        // The last glob part is **, which matches everything.
                        None => return true
                    };
                    while glob_next.matches(path_part) == MatchResult::NoMatch {
                        path_part = match path_parts.next() {
                            Some(p) => p,
                            // If the last path component did not match the
                            // *next* glob component, then we failed to match.
                            None => return false
                        }
                    }
                }
            }
        }
        // If we matched all parts, return success, else fail.
        return match path_parts.next() {
            None => true,
            Some(_) => false
        };
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
    AnyOf(Vec<String>), // {}

    // TODO: decide whether we want to bother supporting these.
    //AnyWithin(Vec<char>), // []
    //AnyExcept(Vec<char>), // [!]
}

// A single path component of a glob.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct GlobComponent {
    source: String,
    tokens: Vec<GlobToken>
}

#[derive(Debug, Eq, PartialEq)]
pub enum MatchResult {
    NoMatch,
    Match,
    MatchRecurse // i.e. **
}

impl GlobComponent {
    /// Take a path component as string and build a glob component.
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
                    // Detect *?; this would require backtracking so disallow.
                    if let Some(&GlobToken::AnySequence) = tokens.last() {
                        return Err(PathError::InvalidGlobCharacter(
                            "detected *?, which would require backtracking"));
                    }
                    tokens.push(GlobToken::AnyChar);
                    i += 1;
                }
                '*' => {
                    // Detect ** not as whole part or, ***, ****, etc.
                    if chars.len() > i + 1 && chars[i + 1] == '*' {
                        return Err(PathError::InvalidGlobCharacter("detected **"));
                    }
                    tokens.push(GlobToken::AnySequence);
                    i += 1;
                }
                '}' => {
                    return Err(PathError::MismatchedBraces(
                               "found closing { without an opening"));
                }
                '{' => {
                    // Search for the closing }.
                    i += 1;
                    let start = i;
                    while i < chars.len() && chars[i] != '}' {
                        if chars[i] == '{' {
                            return Err(PathError::MismatchedBraces(
                                       "found second { before closing }"));
                        }
                        i += 1;
                    }
                    if i >= chars.len() {
                        return Err(PathError::MismatchedBraces(
                                   "string ends before matching } was found"));
                    }
                    // Split into options for matching.
                    let inner: String = chars[start..i].to_owned().into_iter().collect();
                    if inner.len() == 0 {
                        return Err(PathError::MismatchedBraces(
                                   "the braced content is empty"));
                    }
                    let parts: Vec<String> = inner.split(',').map(|p| p.to_owned()).collect();
                    tokens.push(GlobToken::AnyOf(parts));
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

    /// Returns whether the token matches and whether the match is recursive.
    pub fn matches(&self, name: &str) -> MatchResult {
        let mut part = name.chars();
        let mut tokens = self.tokens.iter();
        'head: loop {
            // Take the next token.
            let token = match tokens.next() {
                None => break,
                Some(t) => t
            };
            match *token {
                GlobToken::AnyRecursiveSequence => { // **
                    return MatchResult::MatchRecurse;
                },
                GlobToken::AnyChar => { // ?
                    if let Some(_) = part.next() {
                        continue;
                    } else {
                        // Out of chars in the string to match against.
                        return MatchResult::NoMatch;
                    }
                },
                GlobToken::AnyOf(ref options) => {
                    // Grab the rest of the token and use startswith against each option.
                    let remainder = part.as_str().to_owned();
                    for option in options {
                        if remainder.starts_with(option) {
                            // Consume the part of |part| that was matching.
                            for _ in 0..option.len() {
                                part.next();
                            }
                            continue 'head;
                        }
                    }
                    return MatchResult::NoMatch;
                },
                GlobToken::Char(token_char) => {
                    if let Some(c) = part.next() {
                        if token_char != c {
                            // Mismatched pattern.
                            return MatchResult::NoMatch;
                        }
                        continue;
                    } else {
                        // Out of chars to match against.
                        return MatchResult::NoMatch;
                    }
                },
                GlobToken::AnySequence => { // *
                    // We process the next token inline instead of recursing to
                    // avoid making a copy. We can do this because we do not
                    // support backtracking currently.
                    let expect_next = match tokens.next() {
                        // If our last token is star, we'll match the rest of
                        // the string regardless, so we can shortcut here and
                        // return early without having to drain all chars.
                        None => return MatchResult::Match,
                        Some(t) => match *t {
                            GlobToken::Char(c) => c,
                            _ => { panic!("reached *? state"); }
                        }
                    };
                    // If there are no constant chars anywhere after the *, we
                    // have already exited.
                    while let Some(c) = part.next() {
                        if c == expect_next {
                            // Found it, move on to our next token.
                            continue 'head;
                        }
                    }
                    // We ran out of tokens in |part| before finding our
                    // expected next character.
                    return MatchResult::NoMatch;
                }
            }
        }
        // Off the end of the token stream means we did not fail to match
        // against tokens, but we still must have consumed all of our input.
        return if let Some(_) = part.next() {
            MatchResult::NoMatch
        } else {
            MatchResult::Match
        }
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

    // Ensure that various examples of bad paths fail with the right error code.
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
        ("InvalidWhitespaceCharacter", test_whitespace_space, "/foo/a b/baz"),
        ("InvalidWhitespaceCharacter", test_whitespace_nbsp, "/foo/a\u{A0}b/baz"),
        ("InvalidControlCharacter", test_whitespace_tab, "/foo/a\tb/baz"),
        ("InvalidControlCharacter", test_whitespace_vertical_tab, "/foo/a\x0Bb/baz"),
        ("InvalidControlCharacter", test_whitespace_newline, "/foo/a\nb/baz"),
        ("InvalidControlCharacter", test_whitespace_carriage_return, "/foo/a\rb/baz"),
        ("InvalidCharacter", test_invalid_backslash, "/foo/a\\b/baz"),
        ("InvalidCharacter", test_invalid_colon, "/foo/a:b/baz"),
        ("InvalidCharacter", test_invalid_open_bracket, "/foo/a[b/baz"),
        ("InvalidCharacter", test_invalid_close_bracket, "/foo/a]b/baz"),
        ("InvalidCharacter", test_invalid_exclamation, "/foo/a!b/baz"),
        ("InvalidGlobCharacter", test_invalid_star, "/foo/a*b/baz"),
        ("InvalidGlobCharacter", test_invalid_question, "/foo/a?b/baz")
    ]);

    fn make_glob(p: &str) -> Glob {
        PathBuilder::new(p).unwrap().finish_glob().unwrap()
    }

    // Ensure that various examples of bad glob syntax fails with the right error.
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
        ("InvalidGlobCharacter", test_invalid_multistar2_start, "/a/**foo/b"),
        ("InvalidGlobCharacter", test_invalid_multistar2_end, "/a/foo**/b"),
        ("InvalidGlobCharacter", test_invalid_multistar2_middle, "/a/fo**oo/b"),
        ("InvalidGlobCharacter", test_invalid_multistar3, "/a/***/b"),
        ("InvalidGlobCharacter", test_invalid_multistar4, "/a/****/b"),
        ("InvalidGlobCharacter", test_invalid_backtracking1, "/a/foo*?/b"),
        ("InvalidGlobCharacter", test_invalid_backtracking2, "/a/*?foo/b"),
        ("MismatchedBraces", test_mismatched_empty_braces, "/foo/{}/baz"),
        ("MismatchedBraces", test_mismatched_no_closing, "/a/{foo,bar,baz"),
        ("MismatchedBraces", test_mismatched_no_opening1, "/a/foo,bar,baz}"),
        ("MismatchedBraces", test_mismatched_no_opening2, "/a/{foo,bar,baz}}"),
        ("MismatchedBraces", test_mismatched_no_recursion, "/a/{{}")
    ]);

    // Generic glob construction test to make sure that sane combinations don't crash.
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
                    for path_str in success.iter() {
                        let path = make_path(path_str);
                        assert!(glob.matches(&path));
                    }
                    for path_str in failure.iter() {
                        let path = make_path(path_str);
                        assert!(!glob.matches(&path));
                    }
                }
            )*
        }
    }
    make_glob_match_tests!([
        (test_exact,    "/foo", ["/foo"], ["/Xfoo", "/fooX", "/FOO"]),
        (test_q_start,  "/?a", ["/Xa"], ["/ab", "/Xaa"]),
        (test_q_end,    "/a?", ["/aX", "/a."], ["/Xa", "/aXX", "/AX"]),
        (test_q_middle, "/a?b", ["/aXb", "/a.b"], ["/XaXb", "/aXXb", "/aXbX"]),
        (test_s_start,  "/*a", ["/a", "/Xa", "/XXa", "/XXXXXXXa", "/ABCDEFa"],
                               ["/aX", "/XXaX"]),
        (test_s_end,    "/a*", ["/a", "/aX", "/aXXXXX", "/aABCDEF"],
                               ["/Xa", "/XaXXXXX", "/XaABCDEF"]),
        (test_s_middle, "/a*b", ["/ab", "/aXb", "/aXXXXXb", "/aABCDEFb"],
                                ["/Xab", "/abX", "/XaXb", "/aXbX"]),
        (test_ss,       "/**", ["/", "/X", "/X/Y", "/X/Y/Z"], []),
        (test_ss_start, "/**/foo", ["/foo", "/X/foo", "/X/Y/foo", "/X/Y/Z/foo"],
                                   ["/foo/X", "/X/foo/X", "/X/Y/Z/bar"]),
        (test_ss_end,   "/foo/**", ["/foo", "/foo/X", "/foo/X/Y", "/foo/X/Y/Z"],
                                   ["/X/foo", "/X/foo/X", "/X/foo/X/Y"]),
        (test_ss_middle,"/foo/**/bar", ["/foo/bar", "/foo/X/bar",
                                        "/foo/X/Y/bar", "/foo/X/Y/Z/bar"],
                                       ["/X/foo/bar", "/foo/bar/X",
                                        "/X/foo/X/Y/bar", "/foo/X/Y/bar/Z"]),
        (test_any_seq0, "/foo/{a,b}/bar", ["/foo/a/bar", "/foo/b/bar"],
                                          ["/foo/Xa/bar", "/foo/Xb/bar",
                                           "/foo/aX/bar", "/foo/bX/bar",
                                           "/foo/X/bar", "/foo/aa/bar", "/foo/bb/bar",
                                           "/foo/ab/bar", "/foo/ba/bar",
                                           "/a/bar", "/b/bar",
                                           "/foo/a", "/foo/b"]),
        (test_any_seq1, "/foo/X{a,b}/bar", ["/foo/Xa/bar", "/foo/Xb/bar"],
                                           ["/foo/a/bar", "/foo/b/bar",
                                            "/foo/aX/bar", "/foo/bX/bar",
                                            "/foo/X/bar", "/foo/XX/bar",
                                            "/foo/Xaa/bar", "/foo/Xbb/bar",
                                            "/foo/Xab/bar", "/foo/Xba/bar",
                                            "/Xa/bar", "/Xb/bar",
                                            "/foo/Xa", "/bar/Xb"]),
        (test_any_seq2, "/foo/{a,b}X/bar", ["/foo/aX/bar", "/foo/bX/bar"],
                                           ["/foo/a/bar", "/foo/b/bar",
                                            "/foo/Xa/bar", "/foo/Xb/bar",
                                            "/foo/X/bar", "/foo/XX/bar",
                                            "/foo/aaX/bar", "/foo/bbX/bar",
                                            "/foo/abX/bar", "/foo/baX/bar",
                                            "/Xa/bar", "/Xb/bar",
                                            "/foo/Xa", "/bar/Xb"])
    ]);
}
