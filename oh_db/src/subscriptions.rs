// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use glob::Pattern;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::path::{Path};
use ws::util::Token;
use ::SubscriptionId;

make_error!(SubscriptionError; {
    NoSuchSubscription => SubscriptionId
});
pub type SubscriptionResult<T> = Result<T, SubscriptionError>;

/// The collection of observed patterns and who to notify when a path
/// matching one of the patterns changes.
pub struct Subscriptions {
    globs: HashMap<Pattern, GlobSubscriptions>
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


impl Subscriptions {
    pub fn new() -> Subscriptions { Subscriptions { globs: HashMap::new() } }

    pub fn add_subscription(&mut self, sid: &SubscriptionId,
                            conn: &Token, glob: &Pattern)
    {
        let subs = self.get_subscription_set(conn, glob);
        let is_new = subs.layout.insert(*sid);
        assert!(is_new);
    }

    /// Return a vector containing all subscriptions that match the given path.
    pub fn get_subscriptions_for(&self, path: &Path) -> Vec<(Token, SubscriptionId)> {
        for (glob, subs) in self.globs.iter() {
            if glob.matches_path(path) {
                return subs.get_subscriptions_for();
            }
        }
        return Vec::new();
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
    fn get_subscription_set(&mut self, conn: &Token, glob: &Pattern) -> &mut SubscriptionSet {
        if !self.globs.contains_key(glob) {
            self.globs.insert(glob.clone(), GlobSubscriptions::new());
        }
        return self.globs.get_mut(glob).unwrap().get_subscription_set(conn);
    }

}

impl GlobSubscriptions {
    fn new() -> GlobSubscriptions { GlobSubscriptions { connections: HashMap::new() } }

    fn get_subscriptions_for(&self) -> Vec<(Token, SubscriptionId)> {
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
