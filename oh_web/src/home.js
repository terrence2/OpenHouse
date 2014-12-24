function Connection(sock) {
    this.socket = sock;
    this.token = 0;
    this.message_map = new Map();
}
Connection.prototype.query = function(q) {
    var conn = this;
    function query_ready(accept, reject) {
        var token = ++conn.token;
        var query = {query: q, transforms: []};
        var msg = {token: token, message: {type: 'query', query_group: [query]}};
        conn.message_map.set(token, {accept: accept, reject: reject});
        conn.socket.send(JSON.stringify(msg));
    }
    return new Promise(query_ready);
};

function connect(address, broadcast_handler) {
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
                    // Bcast case:
                    var path = data.path;
                    broadcast_handler(path, message);
                }
            }
            accept(conn);
        }
    });
}

exports.connect = connect;
