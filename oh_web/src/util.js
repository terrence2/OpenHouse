// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
function assert(cond) {
    if (!cond)
        throw "Assertion failure";
}

module.exports = {
    assert: assert
};
