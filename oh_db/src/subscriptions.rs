// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use path::{Glob, Path};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use ws::util::Token;
use ::SubscriptionId;

make_error!(SubscriptionError; {
    NoSuchSubscription => SubscriptionId
});
pub type SubscriptionResult<T> = Result<T, SubscriptionError>;

/// The collection of observed patterns and who to notify when a path
/// matching one of the patterns changes.
pub struct Subscriptions {
    globs: HashMap<Glob, GlobSubscriptions>
}

// A single connection may listen to the same glob in multiple locations,
// so a single glob has to be able to map to a set of subscription ids.
struct GlobSubscriptions {
    connections: HashMap<Token, SubscriptionSet>
}

// A set of subscription ids.
struct SubscriptionSet {
    layout: HashSet<SubscriptionId>
}

/// Subscriptions are stored nested for efficient add/remove. On events
/// we flatten them out into connection/sid pairs for sending.
pub type Subscriber = (Token, SubscriptionId);
pub type SubscriberVec = Vec<Subscriber>;

/// A single subscription match maps matching paths to all subscribers that
/// need to be notified with those paths.
pub type SubscriptionMatch = (Vec<Path>, Vec<Subscriber>);
pub type SubscriptionMatches = Vec<SubscriptionMatch>;

impl Subscriptions {
    pub fn new() -> Subscriptions { Subscriptions { globs: HashMap::new() } }

    pub fn add_subscription(&mut self, sid: &SubscriptionId,
                            conn: &Token, glob: &Glob)
    {
        let subs = self.get_subscription_set(conn, glob);
        let is_new = subs.layout.insert(*sid);
        assert!(is_new);
    }

    /// Search the active subscriptions for the given glob and matching paths.
    /// Returns pairs of path vectors and the subscribers that need notified
    /// with those paths.
    pub fn get_matching_subscriptions(&self, _: Option<&Glob>, paths: &[Path])
        -> SubscriptionMatches
    {
        let mut sub_matches: SubscriptionMatches = Vec::new();
        for (glob, subs) in self.globs.iter() {
            let mut matching: Vec<Path> = Vec::new();
            for path in paths {
                if glob.matches(path) {
                    matching.push(path.clone())
                }
            }
            if matching.len() > 0 {
                sub_matches.push((matching, subs.get_subscribers()))
            }
        }
        return sub_matches;
    }

    /// Returns true if the layout sid was present and removed successfully.
    pub fn remove_subscription(&mut self, sid: &SubscriptionId)
        -> SubscriptionResult<()>
    {
        for (_, glob_subs) in self.globs.iter_mut() {
            for (_, subs) in glob_subs.connections.iter_mut() {
                if subs.layout.remove(sid) {
                    return Ok(());
                }
            }
        }
        return Err(SubscriptionError::NoSuchSubscription(*sid));
    }

    /// Remove all uses of the given connection and all subscriptions therein.
    pub fn remove_connection(&mut self, conn: &Token) {
        for (_, glob_subs) in self.globs.iter_mut() {
            glob_subs.connections.remove(conn);
        }
    }

    // Return the subscription set at path:conn, creating it if it doesn't exist.
    fn get_subscription_set(&mut self, conn: &Token, glob: &Glob) -> &mut SubscriptionSet {
        if !self.globs.contains_key(glob) {
            self.globs.insert(glob.clone(), GlobSubscriptions::new());
        }
        return self.globs.get_mut(glob).unwrap().get_subscription_set(conn);
    }

}

impl GlobSubscriptions {
    fn new() -> GlobSubscriptions { GlobSubscriptions { connections: HashMap::new() } }

    fn get_subscribers(&self) -> Vec<(Token, SubscriptionId)> {
        let mut out = Vec::new();
        for (conn, subs) in &self.connections {
            for sid in &subs.layout {
                out.push((*conn, *sid));
            }
        }
        return out;
    }

    fn get_subscription_set(&mut self, conn: &Token) -> &mut SubscriptionSet {
        if !self.connections.contains_key(conn) {
            self.connections.insert(*conn, SubscriptionSet::new());
        }
        return self.connections.get_mut(conn).unwrap();
    }
}

impl SubscriptionSet {
    fn new() -> SubscriptionSet {
        SubscriptionSet {
            layout: HashSet::new()
        }
    }
}
