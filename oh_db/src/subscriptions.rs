// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use message::{KeysSubscriptionId, LayoutSubscriptionId};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use ws::util::Token;

make_error!(SubscriptionError; {
    NoSuchLayoutSubscription => LayoutSubscriptionId,
    NoSuchKeysSubscription => KeysSubscriptionId,
    NodeContainsSubscriptions => String
});
pub type SubscriptionResult<T> = Result<T, SubscriptionError>;

pub struct Subscriptions {
    paths: HashMap<PathBuf, PathSubscriptions>
}

struct PathSubscriptions {
    connections: HashMap<Token, SubscriptionSet>
}

struct SubscriptionSet {
    layout: HashSet<LayoutSubscriptionId>,
    keys: HashSet<KeysSubscriptionId>
}

impl Subscriptions {
    pub fn new() -> Subscriptions { Subscriptions { paths: HashMap::new() } }

    pub fn add_layout_subscription(&mut self, sid: &LayoutSubscriptionId,
                                   conn: &Token, path: &Path)
    {
        let subs = self.get_subscription_set(conn, path);
        let is_new = subs.layout.insert(*sid);
        assert!(is_new);
    }

    pub fn add_keys_subscription(&mut self, sid: &KeysSubscriptionId,
                                 conn: &Token, path: &Path)
    {
        let subs = self.get_subscription_set(conn, path);
        let is_new = subs.keys.insert(*sid);
        assert!(is_new);
    }

    pub fn get_layout_subscriptions_for(&self, path: &Path) -> Vec<(Token, LayoutSubscriptionId)> {
        return match self.paths.get(path) {
            Some(path_subs) => path_subs.get_layout_subscriptions_for(),
            None => Vec::new()
        };
    }

    pub fn get_keys_subscriptions_for(&self, path: &Path) -> Vec<(Token, KeysSubscriptionId)> {
        return match self.paths.get(path) {
            Some(path_subs) => path_subs.get_keys_subscriptions_for(),
            None => Vec::new()
        };
    }

    pub fn verify_no_subscriptions_at_path(&self, path: &Path) -> SubscriptionResult<()> {
        return match self.paths.contains_key(path) {
            true => Err(SubscriptionError::NodeContainsSubscriptions(path.to_string_lossy().into_owned())),
            false => Ok(())
        };
    }

    /// Returns true if the layout sid was present and removed successfully.
    pub fn remove_layout_subscription(&mut self, sid: &LayoutSubscriptionId)
        -> SubscriptionResult<()>
    {
        for (_, path_subs) in self.paths.iter_mut() {
            for (_, subs) in path_subs.connections.iter_mut() {
                if subs.layout.remove(sid) {
                    return Ok(());
                }
            }
        }
        return Err(SubscriptionError::NoSuchLayoutSubscription(*sid));
    }

    /// Returns true if the layout sid was present and removed successfully.
    pub fn remove_keys_subscription(&mut self, sid: &KeysSubscriptionId)
        -> SubscriptionResult<()>
    {
        for (_, path_subs) in self.paths.iter_mut() {
            for (_, subs) in path_subs.connections.iter_mut() {
                if subs.keys.remove(sid) {
                    return Ok(());
                }
            }
        }
        return Err(SubscriptionError::NoSuchKeysSubscription(*sid));
    }

    /// Remove all uses of the given connection and all subscriptions therein.
    pub fn remove_connection(&mut self, conn: &Token) {
        for (_, path_subs) in self.paths.iter_mut() {
            path_subs.connections.remove(conn);
        }
    }

    // Return the subscription set at path:conn, creating it if it doesn't exist.
    fn get_subscription_set(&mut self, conn: &Token, path: &Path) -> &mut SubscriptionSet {
        if !self.paths.contains_key(path) {
            self.paths.insert(path.to_owned(), PathSubscriptions::new());
        }
        return self.paths.get_mut(path).unwrap().get_subscription_set(conn);
    }

}

impl PathSubscriptions {
    fn new() -> PathSubscriptions { PathSubscriptions { connections: HashMap::new() } }

    fn get_layout_subscriptions_for(&self) -> Vec<(Token, LayoutSubscriptionId)> {
        let mut out = Vec::new();
        for (conn, subs) in &self.connections {
            for sid in &subs.layout {
                out.push((*conn, *sid));
            }
        }
        return out;
    }

    fn get_keys_subscriptions_for(&self) -> Vec<(Token, KeysSubscriptionId)> {
        let mut out = Vec::new();
        for (conn, subs) in &self.connections {
            for sid in &subs.keys {
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
            layout: HashSet::new(),
            keys: HashSet::new()
        }
    }
}
