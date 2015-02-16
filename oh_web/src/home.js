// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
function Query(conn, query_text) {
    this.connection = conn;
    this.query_text = query_text;
    this.transforms = [];
}
Query.prototype.attr = function(key, value) {
    this.transforms.push({
        method: 'attr',
        args: [key, value]
    });
    return this;
};
Query.prototype.run = function() {
    return this.connection._do_query(this);
};
Query.prototype.get_query = function() {
    return this.query_text;
};
Query.prototype.get_transforms = function() {
    return this.transforms;
};


function Connection(sock) {
    this.socket = sock;
    this.token = 0;
    this.message_map = new Map();
    this.subscriptions = new Map();
}
Connection.prototype.query = function(q) {
    return new Query(this, q);
};
Connection.prototype._do_query = function(q) {
    var conn = this;
    function query_ready(accept, reject) {
        var token = ++conn.token;
        var query = {query: q.get_query(), transforms: q.get_transforms()};
        var msg = {token: token, message: {type: 'query', query_group: [query]}};
        conn.message_map.set(token, {accept: accept, reject: reject});
        conn.socket.send(JSON.stringify(msg));
    }
    return new Promise(query_ready);
};
Connection.prototype.subscribe = function(path, callback) {
    var conn = this;
    var callbacks = conn.subscriptions.get(path);
    if (callbacks === undefined)
        callbacks = [];
    callbacks.push(callback);
    conn.subscriptions.set(path, callbacks);
};


function connect(address) {
    return new Promise(function(accept, reject) {
        var socket = new WebSocket(address, "JSON");

        socket.onopen = function(e) {
            var conn = new Connection(socket);
            socket.onmessage = function(e) {
                var data = JSON.parse(e.data);
                var token = data.token;
                var message = data.message;

                if (token !== undefined) {
                    // Reply case:
                    var callbacks = conn.message_map.get(token);
                    if (message.error !== undefined)
                        callbacks.reject(message);
                    callbacks.accept(message);
                    conn.message_map.delete(token);
                } else {
                    // Subscription case:
                    var path = data.path;
                    var callbacks = conn.subscriptions.get(path);
                    if (callbacks !== undefined) {
                        for (var i = 0; i < callbacks.length; ++i)
                            callbacks[i](path, message);
                    }
                }
            }
            accept(conn);
        }
    });
}

exports.connect = connect;
