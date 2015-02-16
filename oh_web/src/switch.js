// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
var $ = require('jquery');
var R = require('ramda');
var jss = require('jss');
var home = require('./home');


function attach(conn, elem)
{
    var styles = jss.createStyleSheet({});
    styles.attach();

    $(elem).append('<div class="scene_select">Scene Select: '+
                   '<select id="global_switch"></select></div>');
    var gswitch = $('#global_switch');

    // Load the set of available options.
    conn.query('scene').run()
        .then(R.values)
        .then(R.map((data) => data.attrs.name))
        .then(R.map((name) => `<option value="${name}">${name}</option>`))
        .then(R.map((opt) => $(gswitch).append(opt)))
        .then((_) => {
            conn.query('home').run()
                .then(msg => {
                    // Set initial switch state.
                    var path = R.last(R.keys(msg));
                    var data = R.last(R.values(msg));
                    $(gswitch).val(data.attrs.scene);

                    // Monitor switch for future, external changes.
                    conn.subscribe(path, (pathArg, msg) => {
                        $(gswitch).val(msg.attrs.scene);
                    });
                });
        });

    gswitch.on('change', (e) => {
        var switchValue = e.target.options[e.target.selectedIndex].value;
        console.log("new switch val: " + switchValue);
        conn.query('home').attr('scene', switchValue).run();
    });
}

module.exports = {
    attach: attach
};
