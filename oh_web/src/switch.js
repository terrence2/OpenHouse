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

    var gpath = undefined;  // Path of scene we're observing.
    function broadcast_handler(path, msg) {
        if (path === gpath)
            $(gswitch).val(msg.attrs.scene);
    }

    conn.query('[kind=scene]').run()
        .then(R.values)
        .then(R.map((data) => data.attrs.name))
        .then(R.map((name) => `<option value="${name}">${name}</option>`))
        .then(R.map((opt) => $(gswitch).append(opt)))
        .then((_) => {
            conn.query('[kind=home]').run()
                .then(data => {
                    gpath = R.last(R.keys(data));
                    $(gswitch).val(R.last(R.values(data)).attrs.scene);
                });
        });

    gswitch.on('change', (e) => {
        var switchValue = e.target.options[e.target.selectedIndex].value;
        conn.query('[kind=home]').attr('scene', switchValue).run();
    });

    return broadcast_handler;
}

module.exports = {
    attach: attach
};
