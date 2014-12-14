var R = require('ramda');
var $ = require('jquery');
var jss = require('jss');
var home = require('./home');


function assert(cond) {
    if (!cond)
        throw "Assertion failure";
}

function attach(tree, parent) {
    $(parent).append("<ul></ul>");
    for (var key in tree.children) {
        var child = tree.children[key];
        var p = $(parent)
                    .children()
                    .first().append(
                      $("<li>").attr('id', key + '-item')
                               .addClass('tree-item-style')
                               .addClass(child.data.attrs.kind + '-style').append(
                        $("<span>").html(key)
                      ).append(
                        $("<div>").attr('id', key + '-child')
                      )
                    );
        attach(child, $("#" + key + "-child"));
        debugger;
    }
    return;
}

function first_component(path) {
    assert(path[0] == '/');
    var parts = path.slice(1).split('/');
    return parts[0];
}

function remainder(path) {
    assert(path[0] == '/');
    var parts = path.slice(1).split('/');
    return '/' + parts.slice(1).join('/');
}

function treeify(data, depth) {
    if (depth === undefined)
        depth = 0;

    var keys = R.sort(R.less, R.keys(data));
    if (keys.length === 0)
        return {};

    var children = {}
    var own_data = undefined;
    for (var i = 0; i < keys.length; ++i) {
        var key = keys[i];
        var first = first_component(key);
        var rem = remainder(key);

        if (first === '') {
            own_data = data[key];
        } else {
            if (children[first] === undefined)
                children[first] = {};
            children[first][rem] = data[key];
        }
    }

    for (var key in children)
        children[key] = treeify(children[key], depth+1);

    return {children: children, data: own_data};
}

var styles = jss.createStyleSheet({
    '.tree-item-style': {
    },
    '.undefined-style': {
        'list-style': 'disc',
    },
    '.home-style': {
        'list-style-image': 'url(/resources/home32.png)',
    },
    '.room-style': {
        'list-style-image': 'url(/resources/room32.png)',
    },
    '.hue-style': {
        'list-style-image': 'url(/resources/hue32.png)',
    },
    '.huebridge-style': {
        'list-style': 'disc',
    },
    '.wemomotion-style': {
        'list-style-image': 'url(/resources/wemomotion32.png)',
    },
    '.wemoswitch-style': {
        'list-style': 'disc',
    },
    '.scene-style': {
        'list-style': 'disc',
    },
});
styles.attach();

home.connect(HOME_ADDRESS)
    .then(conn => conn.query('div'))
    .then(msg => {
        //R.map(key => $(target).append(`<div>${key}</div>`), R.keys(msg));
        var tree = treeify(msg);
        console.log(tree.toSource());
        $('body').empty().append('<div id="root">/<div></div></div>');
        attach(tree, $("#root > div"));
        //R.map(key => attach(key), keys);
    })
    //.then(R.get('pong'))
    //.then(msg => console.log(msg)); // bug 1105149

