// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use failure::Error;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum PathComponent {
    Name(String),
    Lookup(ScriptPath),
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ScriptPath {
    pub components: Vec<PathComponent>,
}

impl ScriptPath {
    pub fn from_str_at_path(base_path: &str, s: &str) -> Result<Self, Error> {
        assert!(base_path.starts_with('/'));

        let (start, mut components) = if s.starts_with('/') {
            (1, Vec::new())
        } else {
            (
                0,
                (&base_path[1..])
                    .split('/')
                    .map(|c| PathComponent::Name(c.to_owned()))
                    .collect::<Vec<PathComponent>>(),
            )
        };
        Self::parse_parts(&mut components, base_path, &s[start..])?;
        return Ok(ScriptPath { components });
    }

    fn parse_parts(
        components: &mut Vec<PathComponent>,
        base_path: &str,
        s: &str,
    ) -> Result<(), Error> {
        let parts = Self::tokenize_path(s)?;
        for part in parts.iter() {
            Self::parse_part(components, base_path, part)?;
        }
        return Ok(());
    }

    fn parse_part(
        components: &mut Vec<PathComponent>,
        base_path: &str,
        part: &str,
    ) -> Result<(), Error> {
        match part {
            "" => bail!(
                "parse error: empty path component under '{}' in '{:?}'",
                base_path,
                components
            ),
            "." => {}
            ".." => {
                ensure!(
                    components.len() > 0,
                    "parse error: looked up parent dir (..) past start of path at '{}' in '{:?}'",
                    base_path,
                    components
                );
                components.pop();
            }
            s => {
                if s.starts_with('{') && s.ends_with('}') {
                    let c = PathComponent::Lookup(Self::from_str_at_path(
                        base_path,
                        &s[1..s.len() - 1],
                    )?);
                    components.push(c);
                } else {
                    ensure!(!s.contains('{'), "parse error: found { in path part");
                    ensure!(!s.contains('}'), "parse error: found } in path part");
                    let c = PathComponent::Name(s.to_owned());
                    components.push(c);
                }
            }
        }
        return Ok(());
    }

    fn tokenize_path(s: &str) -> Result<Vec<String>, Error> {
        let mut brace_depth = 0;
        let mut part_start = 0;
        let mut offset = 0;
        let mut parts = Vec::new();
        for c in s.chars() {
            match c {
                '/' => {
                    if brace_depth == 0 {
                        parts.push(s[part_start..offset].chars().collect::<String>());
                        part_start = offset + 1;
                    }
                }
                '{' => {
                    brace_depth += 1;
                }
                '}' => {
                    brace_depth -= 1;
                }
                _ => {}
            }
            offset += 1;
        }
        ensure!(
            brace_depth == 0,
            "parse error: mismatched braces in path '{}'",
            s
        );
        parts.push(s[part_start..offset].chars().collect::<String>());
        return Ok(parts);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    #[should_panic]
    fn test_parse_invalid_path_embedded_empty() {
        ScriptPath::from_str_at_path("/", "/foo/{/baz//bep}/bar").unwrap();
    }

    #[test]
    #[should_panic]
    fn test_parse_invalid_path_mismatched_open() {
        ScriptPath::from_str_at_path("/", "/foo/{/baz/bep").unwrap();
    }

    #[test]
    #[should_panic]
    fn test_parse_invalid_path_mismatched_close() {
        ScriptPath::from_str_at_path("/", "/foo/{/baz/bep}}").unwrap();
    }

    fn n(p: &str) -> PathComponent {
        PathComponent::Name(p.to_owned())
    }

    fn p(v: Vec<PathComponent>) -> PathComponent {
        PathComponent::Lookup(ScriptPath { components: v })
    }

    #[test]
    fn test_parse_abs_deep_nest() {
        let path = ScriptPath::from_str_at_path("/", "/a/{/0/{/A/B}/2}/c").unwrap();
        assert_eq!(
            path.components,
            vec![
                n("a"),
                p(vec![n("0"), p(vec![n("A"), n("B")]), n("2")]),
                n("c"),
            ]
        )
    }

    #[test]
    fn test_parse_abs_current() {
        let path = ScriptPath::from_str_at_path("/", "/foo/./bar").unwrap();
        assert_eq!(path.components, vec![n("foo"), n("bar")])
    }

    #[test]
    fn test_parse_abs_parent() {
        let path = ScriptPath::from_str_at_path("/", "/foo/../bar").unwrap();
        assert_eq!(path.components, vec![n("bar")])
    }

    #[test]
    fn test_parse_abs_embedded_abs_current() {
        let path = ScriptPath::from_str_at_path("/", "/foo/{/baz/./bep}/bar").unwrap();
        assert_eq!(
            path.components,
            vec![n("foo"), p(vec![n("baz"), n("bep")]), n("bar")]
        )
    }

    #[test]
    fn test_parse_abs_embedded_abs_parent() {
        let path = ScriptPath::from_str_at_path("/", "/foo/{/baz/../bep}/bar").unwrap();
        assert_eq!(path.components, vec![n("foo"), p(vec![n("bep")]), n("bar")])
    }

    #[test]
    fn test_parse_rel_current() {
        let path = ScriptPath::from_str_at_path("/a/b", "./c/d").unwrap();
        assert_eq!(path.components, vec![n("a"), n("b"), n("c"), n("d")])
    }

    #[test]
    fn test_parse_rel_parent() {
        let path = ScriptPath::from_str_at_path("/a/b", "../c/d").unwrap();
        assert_eq!(path.components, vec![n("a"), n("c"), n("d")])
    }

    #[test]
    #[should_panic]
    fn test_parse_rel_parent_underflow() {
        ScriptPath::from_str_at_path("/a/b", "../c/../../../d").unwrap();
    }

    #[test]
    fn test_parse_rel_embedded_rel_parent() {
        let path = ScriptPath::from_str_at_path("/a/b", "../c/{../e}/d").unwrap();
        assert_eq!(
            path.components,
            vec![n("a"), n("c"), p(vec![n("a"), n("e")]), n("d")]
        )
    }
}
