// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
use yggdrasil::Glob;
use yggdrasil::TreeChanges;
use ws::util::Token;
use SubscriptionId;
use std::collections::HashMap;

pub mod errors {
    error_chain!{}
}
use subscriptions::errors::Result;

#[derive(Debug, Clone)]
struct Watch {
    glob: Glob,
    conn: Token,
    sid: SubscriptionId,
}

pub struct Watches {
    watches: Vec<Watch>,
}

impl Watches {
    pub fn new() -> Self {
        Watches { watches: Vec::new() }
    }

    pub fn add_watch(&mut self, sid: &SubscriptionId, conn: &Token, glob: &Glob) {
        // Assert that at least the sid is unique.
        for watch in &self.watches {
            debug_assert!(watch.sid != *sid);
        }

        self.watches.push(Watch {
            glob: glob.clone(),
            conn: *conn,
            sid: *sid,
        });
    }

    pub fn remove_watch(&mut self, sid: &SubscriptionId) -> Result<()> {
        let next_watches: Vec<Watch> = self.watches
            .iter()
            .filter(|w| w.sid != *sid)
            .map(|w| w.clone())
            .collect::<Vec<_>>();
        if next_watches.len() == self.watches.len() {
            bail!(format!("no such subscription: {}", *sid));
        }
        self.watches = next_watches;
        return Ok(());
    }

    pub fn remove_connection(&mut self, conn: &Token) {
        self.watches = self.watches
            .iter()
            .filter(|w| w.conn == *conn)
            .map(|w| w.clone())
            .collect::<Vec<_>>();
    }

    pub fn filter_changes_for_each_watch(
        &self,
        changes: &TreeChanges,
    ) -> Vec<(TreeChanges, Token, SubscriptionId)> {
        let mut result = Vec::new();
        for watch in &self.watches {
            let mut filtered: Option<TreeChanges> = None;
            for (data, paths) in changes {
                for path in paths {
                    if watch.glob.matches(path) {
                        // Add path to filtered change set.
                        if filtered.is_none() {
                            filtered = Some(HashMap::new());
                        }
                        if let Some(ref mut f) = filtered {
                            if !f.contains_key(data) {
                                f.insert(data.clone(), Vec::new());
                            }
                            if let Some(ref mut v) = f.get_mut(data) {
                                v.push(path.clone());
                            }
                        }
                    }
                }
            }
            if let Some(filt) = filtered {
                result.push((filt, watch.conn, watch.sid));
            }
        }
        return result;
    }
}
